import { useLayoutEffect, useRef } from "react";

export interface Flight {
  id: number;
  coins: number;
  from: DOMRect;
  to: DOMRect;
  /** Balance to show when this coin lands. */
  balance: number;
}

const DURATION = 520;

/** A "+N" coin chip that pops up from the ticked card and flies in an arc to the coin counter. */
export function CoinFly({ flight, onLanded }: { flight: Flight; onLanded(f: Flight): void }) {
  const outer = useRef<HTMLDivElement>(null);
  const inner = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    const o = outer.current!;
    const i = inner.current!;
    const box = i.getBoundingClientRect();
    const { from, to } = flight;
    // Start centered on the checkbox, end centered on the coin counter.
    const x0 = from.left + from.width / 2 - box.width / 2;
    const y0 = from.top + from.height / 2 - box.height / 2;
    const x1 = to.left + to.width / 2 - box.width / 2;
    const y1 = to.top + to.height / 2 - box.height / 2;
    // X and Y use different easing on nested elements, which makes the path curve.
    const move = o.animate([{ transform: `translateX(${x0}px)` }, { transform: `translateX(${x1}px)` }], {
      duration: DURATION,
      easing: "ease-out",
      fill: "forwards",
    });
    const anim = i.animate(
      [
        { transform: `translateY(${y0}px) scale(0.6)`, opacity: 0 },
        { transform: `translateY(${y0 - 26}px) scale(1.1)`, opacity: 1, offset: 0.25 },
        { transform: `translateY(${y1}px) scale(0.7)`, opacity: 0.9 },
      ],
      { duration: DURATION, easing: "ease-in", fill: "forwards" },
    );
    anim.onfinish = () => onLanded(flight);
    return () => {
      anim.onfinish = null;
      anim.cancel();
      move.cancel();
    };
  }, []);

  return (
    <div className="fly" ref={outer} aria-hidden>
      <div className="fly-chip" ref={inner}>
        <i className="coin" />+{flight.coins}
      </div>
    </div>
  );
}
