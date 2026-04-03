import React from "react";

interface DemoVideoProps {
  src?: string;
  poster?: string;
  caption: string;
  alt: string;
  ctaLabel?: string;
  ctaHref?: string;
}

export default function DemoVideo({
  src,
  poster,
  caption,
  alt,
  ctaLabel,
  ctaHref,
}: DemoVideoProps) {
  const isGif = src?.endsWith(".gif");

  return (
    <div className="demo-video-container">
      {src ? (
        <div className="demo-video-frame">
          {isGif ? (
            <img src={src} alt={alt} loading="lazy" />
          ) : (
            <video
              src={src}
              poster={poster}
              autoPlay
              muted
              loop
              playsInline
              aria-label={alt}
            />
          )}
        </div>
      ) : (
        <div className="demo-placeholder">
          <div className="demo-placeholder-icon">▶</div>
          <span>Recording coming soon</span>
        </div>
      )}
      <div className="demo-meta">
        <p className="demo-caption">{caption}</p>
        {ctaLabel && ctaHref && (
          <a className="demo-cta" href={ctaHref}>
            {ctaLabel} →
          </a>
        )}
      </div>
    </div>
  );
}
