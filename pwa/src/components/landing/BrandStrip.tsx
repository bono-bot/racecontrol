export function BrandStrip() {
  return (
    <header className="flex items-center gap-3 border-b border-rp-border bg-rp-dark px-5 py-4">
      <svg
        viewBox="0 0 32 32"
        width="28"
        height="28"
        aria-hidden="true"
        className="shrink-0"
      >
        <path
          d="M4 22 A 12 12 0 0 1 28 22"
          stroke="currentColor"
          strokeWidth="3"
          fill="none"
          className="text-rp-red"
          strokeLinecap="round"
        />
        <circle cx="16" cy="22" r="2" className="fill-rp-red" />
      </svg>
      <span className="font-display text-lg font-bold tracking-wide text-white">
        RACING POINT
      </span>
    </header>
  );
}
