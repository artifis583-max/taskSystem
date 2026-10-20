import { useState } from "react";
import { newOpId, type Api } from "./api";
import type { AppState, Reward, RewardInput } from "./types";

interface Props {
  api: Api;
  state: AppState;
  onError(msg: string): void;
  onBanner(msg: string): void;
}

type Confirm = { kind: "buy" } | { kind: "revive" } | { kind: "redeem"; reward: Reward } | null;

export function RewardsTab({ api, state, onError, onBanner }: Props) {
  const [confirm, setConfirm] = useState<Confirm>(null);
  const [busy, setBusy] = useState(false);
  const [open, setOpen] = useState<number | null>(null);
  const [editing, setEditing] = useState<Reward | "new" | null>(null);
  const { revives, balance } = state;
  const haveRevive = revives.freeAvailable || revives.bought > 0;

  async function run(fn: () => Promise<unknown>, success?: string) {
    if (busy) return;
    setBusy(true);
    setConfirm(null);
    try {
      await fn();
      if (success) onBanner(success);
    } catch (e) {
      onError(String(e));
    }
    setBusy(false);
  }

  if (editing) {
    return (
      <RewardForm
        initial={editing === "new" ? undefined : editing}
        onCancel={() => setEditing(null)}
        onSave={(input) =>
          run(async () => {
            if (editing === "new") await api.createReward(input);
            else await api.updateReward(editing.id, input);
            setEditing(null);
          })
        }
      />
    );
  }

  const isConfirm = (c: Confirm) => JSON.stringify(c) === JSON.stringify(confirm);

  return (
    <div className="list shop">
      {revives.brokenDate && (
        <div className="alert">
          {isConfirm({ kind: "revive" }) ? (
            <ConfirmRow
              text={`Use ${revives.freeAvailable ? "your free" : "a bought"} revive?`}
              onYes={() => run(() => api.useRevive(), `Streak saved: ${revives.streakToRestore} days`)}
              onNo={() => setConfirm(null)}
            />
          ) : (
            <>
              <div className="alert-text">
                <strong>Your {revives.streakToRestore}-day streak broke yesterday.</strong>
                <span>{haveRevive ? "Use a revive today to keep it." : "Buy a revive below to keep it. Today only."}</span>
              </div>
              <button className="mini" disabled={!haveRevive || busy} onClick={() => setConfirm({ kind: "revive" })}>
                Revive
              </button>
            </>
          )}
        </div>
      )}

      <h3 className="section">Streak savers</h3>
      <div className="row-card">
        {isConfirm({ kind: "buy" }) ? (
          <ConfirmRow
            text={`Spend ${revives.price} coins on a revive?`}
            onYes={() => run(() => api.buyRevive(newOpId()), "Revive added")}
            onNo={() => setConfirm(null)}
          />
        ) : (
          <>
            <ShieldIcon />
            <div className="row-text">
              <span className="title">
                {revives.freeAvailable ? "Free revive ready" : "Free revive used"}
                {revives.bought > 0 && <span className="count-chip">+{revives.bought} bought</span>}
              </span>
              <span className="details">
                {revives.freeAvailable ? "One per week. Saves a broken streak." : "A new one arrives on Monday."}
              </span>
            </div>
            <button
              className="price-btn"
              disabled={balance < revives.price || busy}
              title={balance < revives.price ? `You need ${revives.price - balance} more coins` : undefined}
              onClick={() => setConfirm({ kind: "buy" })}
            >
              Buy <i className="coin" aria-hidden /> {revives.price}
            </button>
          </>
        )}
      </div>

      <h3 className="section">Your rewards</h3>
      {state.rewards.length === 0 && (
        <p className="hint">Add things you'd like to earn, like a movie night or an hour of gaming.</p>
      )}
      {state.rewards.map((r) => {
        const short = r.price - balance;
        return (
          <div className="row-card reward-card" key={r.id}>
            {isConfirm({ kind: "redeem", reward: r }) ? (
              <ConfirmRow
                text={`Spend ${r.price} coins on ${r.name}?`}
                onYes={() => run(() => api.redeemReward(newOpId(), r.id), `Enjoy: ${r.name}`)}
                onNo={() => setConfirm(null)}
              />
            ) : (
              <>
                <button className="row-text" onClick={() => setOpen(open === r.id ? null : r.id)} aria-expanded={open === r.id}>
                  <span className="title">{r.name}</span>
                  {r.note && <span className="details">{r.note}</span>}
                  {open === r.id && (
                    <span className="actions">
                      <span className="mini ghost" role="button" onClick={(e) => (e.stopPropagation(), setOpen(null), setEditing(r))}>
                        Edit
                      </span>
                      <span
                        className="mini ghost"
                        role="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          setOpen(null);
                          run(() => api.archiveReward(r.id));
                        }}
                      >
                        Remove
                      </span>
                    </span>
                  )}
                </button>
                <div className="redeem">
                  <button
                    className="price-btn"
                    disabled={short > 0 || busy}
                    onClick={() => setConfirm({ kind: "redeem", reward: r })}
                  >
                    <i className="coin" aria-hidden /> {r.price}
                  </button>
                  {short > 0 && <span className="need">{short} more</span>}
                </div>
              </>
            )}
          </div>
        );
      })}
      <button className="add" onClick={() => setEditing("new")}>
        + New reward
      </button>

      {state.redemptions.length > 0 && (
        <>
          <h3 className="section">Recently redeemed</h3>
          <ul className="history">
            {state.redemptions.map((h) => (
              <li key={h.id}>
                <span>{h.name}</span>
                <span className="muted">
                  {h.price} · {relativeDay(h.createdAt)}
                </span>
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}

function ConfirmRow({ text, onYes, onNo }: { text: string; onYes(): void; onNo(): void }) {
  return (
    <div className="confirm-row">
      <span>{text}</span>
      <button className="mini" onClick={onYes} autoFocus>
        Yes
      </button>
      <button className="mini ghost" onClick={onNo}>
        No
      </button>
    </div>
  );
}

function RewardForm({ initial, onSave, onCancel }: { initial?: Reward; onSave(r: RewardInput): void; onCancel(): void }) {
  const [name, setName] = useState(initial?.name ?? "");
  const [note, setNote] = useState(initial?.note ?? "");
  const [price, setPrice] = useState(initial?.price ?? 100);
  const valid = name.trim().length > 0 && price >= 1;
  return (
    <form
      className="form"
      onSubmit={(e) => {
        e.preventDefault();
        if (valid) onSave({ name, note, price });
      }}
    >
      <label>
        Reward
        <input autoFocus maxLength={60} value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Movie night" />
      </label>
      <label>
        Note
        <input maxLength={200} value={note} onChange={(e) => setNote(e.target.value)} placeholder="Optional" />
      </label>
      <label>
        Price in coins
        <input
          type="number"
          min={1}
          max={1000000}
          value={price}
          onChange={(e) => setPrice(Math.max(0, Math.floor(Number(e.target.value) || 0)))}
        />
      </label>
      <div className="seg">
        {[50, 100, 250, 500].map((p) => (
          <button type="button" key={p} className={price === p ? "on" : ""} onClick={() => setPrice(p)}>
            {p}
          </button>
        ))}
      </div>
      <div className="form-actions">
        <button type="button" className="mini ghost" onClick={onCancel}>
          Cancel
        </button>
        <button type="submit" className="mini" disabled={!valid}>
          {initial ? "Save" : "Add reward"}
        </button>
      </div>
    </form>
  );
}

function ShieldIcon() {
  return (
    <svg className="row-icon" viewBox="0 0 24 24" aria-hidden>
      <path d="M12 3l7 3v5c0 4.5-3 8.3-7 10-4-1.7-7-5.5-7-10V6l7-3z" />
      <path d="M8.5 12l2.5 2.5 4.5-5" />
    </svg>
  );
}

function relativeDay(iso: string): string {
  const then = new Date(iso);
  const days = Math.round((startOfDay(new Date()) - startOfDay(then)) / 864e5);
  if (days <= 0) return "today";
  if (days === 1) return "yesterday";
  if (days < 7) return `${days} days ago`;
  return then.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

function startOfDay(d: Date) {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}
