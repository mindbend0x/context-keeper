import React from "react";
import Link from "@docusaurus/Link";

interface DemoVideoProps {
  src?: string;
  poster?: string;
  description?: string;
  caption: string;
  alt: string;
  ctaLabel?: string;
  ctaHref?: string;
}

export default function DemoVideo({
  src,
  poster,
  description,
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
          {description && (
            <p className="demo-placeholder-desc">{description}</p>
          )}
        </div>
      )}
      <div className="demo-meta">
        <p className="demo-caption">{caption}</p>
        {ctaLabel && ctaHref && (
          <Link className="demo-cta" to={ctaHref}>
            {ctaLabel} →
          </Link>
        )}
      </div>
    </div>
  );
}
