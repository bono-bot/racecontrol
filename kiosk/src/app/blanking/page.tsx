"use client";

import { useEffect, useRef } from "react";

export default function BlankingScreen() {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let W: number, H: number, dpr: number;
    const particles: Ember[] = [];
    let animId: number;

    class Ember {
      x = 0; y = 0; vx = 0; vy = 0; r = 0;
      life = 1; decay = 0; hue = 0;

      constructor(init: boolean) { this.reset(init); }

      reset(init: boolean) {
        this.x = W * 0.15 + Math.random() * W * 0.7;
        this.y = init ? Math.random() * H : H + 10;
        this.vx = (Math.random() - 0.5) * 0.3;
        this.vy = -(Math.random() * 0.6 + 0.2);
        this.r = Math.random() * 2.2 + 0.6;
        this.life = 1;
        this.decay = Math.random() * 0.003 + 0.001;
        this.hue = Math.random() > 0.7 ? 20 : 0;
      }

      update() {
        this.x += this.vx + Math.sin(Date.now() * 0.0008 + this.x * 0.01) * 0.15;
        this.y += this.vy;
        this.life -= this.decay;
        if (this.life <= 0 || this.y < -20) this.reset(false);
      }

      draw(c: CanvasRenderingContext2D) {
        const a = this.life * 0.7;
        c.beginPath();
        c.arc(this.x, this.y, this.r, 0, Math.PI * 2);
        c.fillStyle = `hsla(${this.hue}, 90%, 55%, ${a})`;
        c.fill();
        c.beginPath();
        c.arc(this.x, this.y, this.r * 3.5, 0, Math.PI * 2);
        c.fillStyle = `hsla(${this.hue}, 90%, 50%, ${a * 0.15})`;
        c.fill();
      }
    }

    function resize() {
      dpr = window.devicePixelRatio || 1;
      W = window.innerWidth;
      H = window.innerHeight;
      canvas!.width = W * dpr;
      canvas!.height = H * dpr;
      canvas!.style.width = W + "px";
      canvas!.style.height = H + "px";
      ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);
    }

    function init() {
      resize();
      for (let i = 0; i < 55; i++) particles.push(new Ember(true));
    }

    function frame() {
      ctx!.clearRect(0, 0, W, H);
      for (const p of particles) { p.update(); p.draw(ctx!); }
      animId = requestAnimationFrame(frame);
    }

    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (!reducedMotion) {
      init();
      frame();
    }

    window.addEventListener("resize", resize);
    return () => {
      window.removeEventListener("resize", resize);
      cancelAnimationFrame(animId);
    };
  }, []);

  return (
    <div className="blanking-root">
      {/* Particle canvas */}
      <canvas ref={canvasRef} className="fx-canvas" />

      {/* Light beams */}
      <div className="beams">
        <div className="beam beam-1" />
        <div className="beam beam-2" />
        <div className="beam beam-3" />
      </div>

      {/* Floor */}
      <div className="floor" />

      {/* Fog */}
      <div className="fog" />

      {/* Vignette */}
      <div className="vignette" />

      {/* Logo + Reflection */}
      <div className="logo-stage">
        <div className="logo-wrap">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img src="/kiosk/rp-logo-blanking.png" alt="Racing Point Esports" />
        </div>
        <div className="reflection">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img src="/kiosk/rp-logo-blanking.png" alt="" aria-hidden="true" />
        </div>
      </div>

      {/* Scanlines */}
      <div className="scanlines" />

      <style jsx>{`
        .blanking-root {
          position: fixed;
          inset: 0;
          background: #050508;
          overflow: hidden;
          cursor: none;
          user-select: none;
        }

        .fx-canvas {
          position: fixed;
          inset: 0;
          z-index: 1;
          pointer-events: none;
        }

        .vignette {
          position: fixed;
          inset: 0;
          z-index: 2;
          pointer-events: none;
          background: radial-gradient(ellipse 70% 60% at 50% 45%, transparent 40%, rgba(0,0,0,.85) 100%);
        }

        .scanlines {
          position: fixed;
          inset: 0;
          z-index: 5;
          pointer-events: none;
          background: repeating-linear-gradient(0deg, transparent, transparent 2px, rgba(0,0,0,.06) 2px, rgba(0,0,0,.06) 4px);
          opacity: 0.4;
        }

        .floor {
          position: fixed;
          bottom: 0;
          left: 0;
          right: 0;
          height: 45%;
          z-index: 0;
          background: linear-gradient(to bottom, transparent 0%, rgba(10,10,16,.6) 20%, rgba(10,10,16,.95) 60%, #0A0A10 100%);
        }

        .floor::before {
          content: '';
          position: absolute;
          top: 0;
          left: 10%;
          right: 10%;
          height: 1px;
          background: linear-gradient(90deg, transparent, rgba(220,38,38,.15) 20%, rgba(220,38,38,.3) 50%, rgba(220,38,38,.15) 80%, transparent);
        }

        .beams {
          position: fixed;
          inset: 0;
          z-index: 0;
          pointer-events: none;
          overflow: hidden;
        }

        .beam {
          position: absolute;
          top: -10%;
          width: 180px;
          height: 120%;
          background: linear-gradient(180deg, rgba(220,38,38,.07) 0%, rgba(220,38,38,.02) 40%, transparent 70%);
          transform-origin: top center;
          filter: blur(30px);
        }

        .beam-1 { left: 18%; animation: beamSway1 12s ease-in-out infinite; }
        .beam-2 { left: 42%; animation: beamSway2 14s ease-in-out infinite 1s; }
        .beam-3 { right: 18%; animation: beamSway1 16s ease-in-out infinite 3s; }

        .fog {
          position: fixed;
          bottom: 0;
          left: 0;
          right: 0;
          height: 30%;
          z-index: 2;
          pointer-events: none;
          background: radial-gradient(ellipse 80% 100% at 50% 100%, rgba(220,38,38,.06) 0%, transparent 70%);
          animation: fogPulse 8s ease-in-out infinite;
        }

        .logo-stage {
          position: fixed;
          inset: 0;
          z-index: 3;
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          padding-bottom: 6vh;
        }

        .logo-wrap {
          position: relative;
          width: clamp(280px, 38vw, 720px);
          aspect-ratio: 1;
        }

        .logo-wrap img {
          width: 100%;
          height: 100%;
          object-fit: contain;
          filter: drop-shadow(0 0 40px rgba(220,38,38,.35)) drop-shadow(0 0 80px rgba(220,38,38,.15));
          animation: logoPulse 4s ease-in-out infinite;
        }

        .logo-wrap::before {
          content: '';
          position: absolute;
          inset: -12%;
          border-radius: 50%;
          background: radial-gradient(circle, rgba(220,38,38,.08) 0%, transparent 70%);
          animation: ringPulse 4s ease-in-out infinite;
          pointer-events: none;
        }

        .logo-wrap::after {
          content: '';
          position: absolute;
          inset: 15%;
          border-radius: 50%;
          background: radial-gradient(circle, rgba(220,38,38,.12) 0%, transparent 60%);
          animation: ringPulse 4s ease-in-out infinite 2s;
          pointer-events: none;
        }

        .reflection {
          width: clamp(280px, 38vw, 720px);
          aspect-ratio: 1;
          margin-top: -8%;
          transform: scaleY(-1);
          mask-image: linear-gradient(to bottom, rgba(0,0,0,.22) 0%, transparent 55%);
          -webkit-mask-image: linear-gradient(to bottom, rgba(0,0,0,.22) 0%, transparent 55%);
          filter: blur(3px) brightness(.4);
          opacity: 0.5;
          pointer-events: none;
        }

        .reflection img {
          width: 100%;
          height: 100%;
          object-fit: contain;
        }

        @keyframes logoPulse {
          0%, 100% { filter: drop-shadow(0 0 40px rgba(220,38,38,.35)) drop-shadow(0 0 80px rgba(220,38,38,.15)); }
          50%      { filter: drop-shadow(0 0 60px rgba(220,38,38,.55)) drop-shadow(0 0 120px rgba(220,38,38,.25)); }
        }

        @keyframes ringPulse {
          0%, 100% { opacity: 0.6; transform: scale(1); }
          50%      { opacity: 1; transform: scale(1.04); }
        }

        @keyframes beamSway1 {
          0%, 100% { transform: rotate(-6deg); opacity: 0.7; }
          50%      { transform: rotate(4deg); opacity: 1; }
        }

        @keyframes beamSway2 {
          0%, 100% { transform: rotate(3deg); opacity: 0.8; }
          50%      { transform: rotate(-5deg); opacity: 1; }
        }

        @keyframes fogPulse {
          0%, 100% { opacity: 0.5; }
          50%      { opacity: 0.9; }
        }

        @media (prefers-reduced-motion: reduce) {
          .beam, .fog, .logo-wrap img, .logo-wrap::before, .logo-wrap::after {
            animation: none !important;
          }
          .fx-canvas { display: none; }
        }
      `}</style>
    </div>
  );
}
