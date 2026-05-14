/* eslint-disable @next/next/no-img-element */
export function HeroVisual() {
  return (
    <div className="aspect-video w-full overflow-hidden rounded-md border border-rp-border bg-rp-card">
      <img
        src="/hero-photo.webp"
        alt="Driver in a triple-monitor sim racing rig at Racing Point"
        loading="eager"
        decoding="async"
        className="h-full w-full object-cover object-center"
      />
    </div>
  );
}
