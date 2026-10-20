import { useEffect, useRef, useState } from "react";
import { prefersReducedMotion } from "./motion";

/** Shows a number that counts toward `value`, and pulses when it goes up. */
export function CountUp({ value }: { value: number }) {
  const [shown, setShown] = useState(value);
  const [pulse, setPulse] = useState(0);
  const from = useRef(value);

  useEffect(() => {
    const start = from.current;
    from.current = value;
    if (start === value) return;
    if (value > start) setPulse((p) => p + 1);
    if (prefersReducedMotion()) return setShown(value);
    const t0 = performance.now();
    let raf = 0;
    const step = (t: number) => {
      const k = Math.min(1, (t - t0) / 350);
      setShown(Math.round(start + (value - start) * (1 - Math.pow(1 - k, 3))));
      if (k < 1) raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  }, [value]);

  return (
    <span key={pulse} className={pulse ? "count pulse" : "count"}>
      {shown}
    </span>
  );
}
