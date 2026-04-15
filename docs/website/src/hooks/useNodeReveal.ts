import { useEffect, useRef, useCallback } from "react";

export function useNodeReveal() {
  const observerRef = useRef<IntersectionObserver | null>(null);

  useEffect(() => {
    observerRef.current = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (!entry.isIntersecting) return;

          const el = entry.target as HTMLElement;
          const delay = parseInt(el.dataset.revealDelay || "0", 10);

          setTimeout(() => {
            el.classList.add("resolved");
          }, delay);
        });
      },
      { threshold: 0.1 }
    );

    document
      .querySelectorAll(".node-reveal")
      .forEach((el) => observerRef.current?.observe(el));

    return () => observerRef.current?.disconnect();
  }, []);
}

export function useNodeRevealRef<T extends HTMLElement>() {
  const ref = useRef<T>(null);

  const observe = useCallback(() => {
    if (!ref.current) return;

    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (!entry.isIntersecting) return;

          const container = entry.target as HTMLElement;
          const children = container.querySelectorAll(".node-reveal");

          children.forEach((child, i) => {
            const el = child as HTMLElement;
            const baseDelay = parseInt(el.dataset.revealDelay || "0", 10);
            const staggerDelay = i * 80;

            setTimeout(() => {
              el.classList.add("resolved");
            }, baseDelay + staggerDelay);
          });

          observer.unobserve(container);
        });
      },
      { threshold: 0.08 }
    );

    observer.observe(ref.current);

    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const cleanup = observe();
    return cleanup;
  }, [observe]);

  return ref;
}
