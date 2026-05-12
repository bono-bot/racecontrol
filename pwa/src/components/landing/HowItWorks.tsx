type Step = {
  number: string;
  title: string;
  description: string;
  Icon: React.FC<{ className?: string }>;
};

const PhoneIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg viewBox="0 0 24 24" width="28" height="28" className={className} aria-hidden="true">
    <rect
      x="7" y="2.5" width="10" height="19" rx="2"
      stroke="currentColor" strokeWidth="1.5" fill="none"
    />
    <line x1="11" y1="18.5" x2="13" y2="18.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
);

const WheelIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg viewBox="0 0 24 24" width="28" height="28" className={className} aria-hidden="true">
    <circle cx="12" cy="12" r="8.5" stroke="currentColor" strokeWidth="1.5" fill="none" />
    <circle cx="12" cy="12" r="2" stroke="currentColor" strokeWidth="1.5" fill="none" />
    <line x1="12" y1="3.5" x2="12" y2="10" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    <line x1="4.5" y1="15" x2="10.5" y2="13" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    <line x1="19.5" y1="15" x2="13.5" y2="13" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
);

const TrophyIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg viewBox="0 0 24 24" width="28" height="28" className={className} aria-hidden="true">
    <path
      d="M7 4h10v3a5 5 0 0 1-10 0V4z"
      stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinejoin="round"
    />
    <path d="M7 5H4.5v2a3 3 0 0 0 2.5 3" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" />
    <path d="M17 5h2.5v2a3 3 0 0 1-2.5 3" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" />
    <line x1="12" y1="12" x2="12" y2="16" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    <line x1="8.5" y1="20" x2="15.5" y2="20" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    <line x1="10" y1="16" x2="14" y2="16" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
);

const STEPS: Step[] = [
  { number: "01", title: "Register on your phone", description: "Two-minute signup. No app install needed.", Icon: PhoneIcon },
  { number: "02", title: "Walk in. Choose your race.", description: "Pick a car. Pick a circuit. Pick your competition.", Icon: WheelIcon },
  { number: "03", title: "Drive. Climb the leaderboard.", description: "Real wheels, real pedals, real lap times.", Icon: TrophyIcon },
];

export function HowItWorks() {
  return (
    <section className="border-t border-rp-border bg-rp-dark px-5 py-12 sm:px-8 sm:py-16">
      <h2 className="mb-6 font-display text-xl font-bold tracking-wide text-white sm:mb-10 sm:text-2xl">
        HOW IT WORKS
      </h2>
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3 sm:gap-6">
        {STEPS.map(({ number, title, description, Icon }) => (
          <article
            key={number}
            className="rounded-md border border-rp-border bg-rp-card p-5"
          >
            <Icon className="mb-4 text-rp-grey" />
            <div className="mb-2 font-display text-2xl font-bold text-rp-red">{number}</div>
            <h3 className="mb-1 text-base font-semibold text-white sm:text-lg">{title}</h3>
            <p className="text-sm text-rp-grey">{description}</p>
          </article>
        ))}
      </div>
    </section>
  );
}
