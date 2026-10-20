import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { newOpId, type Api } from "./api";
import type { CompleteResult, TaskCard } from "./types";
import { BOUNCE, EASE_OUT, T, prefersReducedMotion, sleep } from "./motion";

type Phase = "ticked" | "swiped" | "landing";

interface Props {
  api: Api;
  cards: TaskCard[];
  /** Coins were saved; `from` is where on screen the coin chip starts its flight. */
  onReward(r: CompleteResult, from: DOMRect | null): void;
  onLanded(r: CompleteResult): void;
  /** All animations finished — safe to reload from the backend. */
  onSettled(): void;
  onError(msg: string): void;
  onAdd(): void;
}

const sameOrder = (a: TaskCard[], b: TaskCard[]) =>
  a.length === b.length &&
  a.every((x, i) => x.occurrenceId === b[i].occurrenceId && x.status === b[i].status && x.coins === b[i].coins);

export function TaskList({ api, cards, onReward, onLanded, onSettled, onError, onAdd }: Props) {
  // What's on screen. Follows `cards`, except while an animation is running.
  const [items, setItems] = useState(cards);
  const [phase, setPhase] = useState<Record<number, Phase>>({});
  const [open, setOpen] = useState<number | null>(null);
  const [confirmUndo, setConfirmUndo] = useState<number | null>(null);

  const busy = useRef(0);
  const queue = useRef<Promise<void>>(Promise.resolve());
  const nodes = useRef(new Map<number, HTMLElement>());
  const flipFrom = useRef<Map<number, number> | null>(null);
  const movers = useRef(new Set<number>());

  const setP = (id: number, p: Phase | null) =>
    setPhase((cur) => {
      const next = { ...cur };
      if (p) next[id] = p;
      else delete next[id];
      return next;
    });

  /** FLIP step 1: remember where every card is before the list changes. */
  function snapshot() {
    const m = new Map<number, number>();
    nodes.current.forEach((el, id) => m.set(id, el.getBoundingClientRect().top));
    flipFrom.current = m;
  }

  // Sync from the backend when nothing is animating.
  useEffect(() => {
    if (busy.current > 0) return;
    setItems((cur) => {
      if (sameOrder(cur, cards)) return cur;
      snapshot();
      return cards;
    });
  }, [cards]);

  // FLIP steps 2–4: after the DOM moved, play each card from its old spot to its new one.
  useLayoutEffect(() => {
    const from = flipFrom.current;
    if (!from) return;
    flipFrom.current = null;
    const reduce = prefersReducedMotion();
    nodes.current.forEach((el, id) => {
      const old = from.get(id);
      if (reduce) return;
      if (old == null) {
        el.animate([{ opacity: 0, transform: "scale(0.96)" }, { opacity: 1, transform: "none" }], {
          duration: 220,
          easing: EASE_OUT,
        });
        return;
      }
      const dy = old - el.getBoundingClientRect().top;
      if (Math.abs(dy) < 1) return;
      const mover = movers.current.has(id);
      if (mover) el.style.zIndex = "2";
      const anim = el.animate([{ transform: `translateY(${dy}px)` }, { transform: "translateY(0)" }], {
        duration: mover ? T.move : T.shift,
        easing: mover ? BOUNCE : EASE_OUT,
      });
      anim.onfinish = anim.oncancel = () => (el.style.zIndex = "");
    });
    movers.current.clear();
  });

  function settleIfIdle() {
    if (busy.current === 0) onSettled();
  }

  async function complete(card: TaskCard) {
    const id = card.occurrenceId;
    if (phase[id] || card.status !== "pending") return;
    const reduce = prefersReducedMotion();
    busy.current++;
    setOpen(null);
    const started = performance.now();
    setP(id, "ticked");
    const request = api.complete(newOpId(), id); // saved right away; animation runs alongside

    let result: CompleteResult;
    try {
      if (!reduce) {
        await sleep(T.tick);
        setP(id, "swiped");
      }
      result = await request;
    } catch (e) {
      setP(id, null);
      busy.current--;
      onError(String(e));
      settleIfIdle();
      return;
    }
    onReward(result, nodes.current.get(id)?.querySelector(".check")?.getBoundingClientRect() ?? null);
    if (!reduce) await sleep(Math.max(0, started + T.tick + T.swipe + T.hold - performance.now()));

    // Moves run one at a time so cards never cross each other.
    const move = async () => {
      snapshot();
      movers.current.add(id);
      setP(id, "landing");
      setItems((cur) => {
        const it = cur.find((c) => c.occurrenceId === id);
        if (!it) return cur;
        const done: TaskCard = { ...it, status: "completed", coins: result.coins, completedSeq: Number.MAX_SAFE_INTEGER };
        return [...cur.filter((c) => c.occurrenceId !== id), done];
      });
      if (!reduce) await sleep(T.move);
      setP(id, null);
      onLanded(result);
    };
    queue.current = queue.current.then(move);
    await queue.current;
    busy.current--;
    settleIfIdle();
  }

  async function uncomplete(card: TaskCard) {
    setConfirmUndo(null);
    busy.current++;
    try {
      await api.uncomplete(newOpId(), card.occurrenceId);
      movers.current.add(card.occurrenceId);
    } catch (e) {
      onError(String(e));
    }
    busy.current--;
    settleIfIdle();
  }

  async function act(fn: () => Promise<void>) {
    setOpen(null);
    try {
      await fn();
    } catch (e) {
      onError(String(e));
    }
  }

  return (
    <div className="list">
      {items.length === 0 && <p className="placeholder">No tasks today.</p>}
      {items.map((card) => {
        const id = card.occurrenceId;
        const p = phase[id];
        const done = card.status === "completed" || !!p;
        const cls = ["slot", p === "swiped" ? "swiped" : "", done ? "done" : ""].join(" ");
        return (
          <div
            key={id}
            className={cls}
            ref={(el) => {
              if (el) nodes.current.set(id, el);
              else nodes.current.delete(id);
            }}
          >
            <div className="done-label" aria-hidden>
              <svg viewBox="0 0 20 16">
                <path d="M3 8.5l4.5 4.5L17 3.5" />
              </svg>
              Done
            </div>
            <div className="card">
              {confirmUndo === id ? (
                <div className="card-text confirm">
                  <span>Undo? You'll lose {card.coins} coins.</span>
                  <button className="mini" onClick={() => uncomplete(card)}>
                    Yes
                  </button>
                  <button className="mini ghost" onClick={() => setConfirmUndo(null)}>
                    No
                  </button>
                </div>
              ) : (
                <button
                  className="card-text"
                  onClick={() => card.status === "pending" && !p && setOpen(open === id ? null : id)}
                  aria-expanded={open === id}
                >
                  <span className="title">{card.title}</span>
                  {card.details && <span className="details">{card.details}</span>}
                  {open === id && (
                    <span className="actions">
                      <span
                        className="mini"
                        role="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          act(() => api.skip(id));
                        }}
                      >
                        Skip today
                      </span>
                      <span
                        className="mini ghost"
                        role="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          act(() => api.removeTask(card.taskId));
                        }}
                      >
                        Remove task
                      </span>
                    </span>
                  )}
                </button>
              )}
              <span className="meta">
                {card.rolloverCount > 0 && card.status === "pending" && (
                  <span className="rollover" title={`Carried over ${card.rolloverCount}×`}>
                    <svg viewBox="0 0 16 16" aria-hidden>
                      <path d="M13 8a5 5 0 1 1-1.5-3.5M13 2.5V5h-2.5" />
                    </svg>
                    {card.rolloverCount}
                  </span>
                )}
              </span>
              <button
                className={done ? "check checked" : "check"}
                aria-label={done ? `Mark "${card.title}" not done` : `Mark "${card.title}" done`}
                aria-pressed={done}
                onClick={() => (card.status === "completed" && !p ? setConfirmUndo(id) : complete(card))}
              >
                <svg viewBox="0 0 20 16" aria-hidden>
                  <path d="M3 8.5l4.5 4.5L17 3.5" />
                </svg>
              </button>
            </div>
          </div>
        );
      })}
      <button className="add" onClick={onAdd}>
        + Add task
      </button>
    </div>
  );
}
