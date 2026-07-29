"use client";

import { useId, useState } from "react";
import Link from "next/link";
import { StatusPill } from "@/components/status-pill";

type RoadmapItemCardProps = {
  title: string;
  status: string;
  voteCount: number;
  commentCount: number;
  href: string;
};

/**
 * A compact roadmap card. By default it shows only the title and status so the
 * list stays dense; a per-item chevron expands it to reveal the metadata
 * (votes / comments) and a link through to the full post.
 */
export function RoadmapItemCard({
  title,
  status,
  voteCount,
  commentCount,
  href,
}: RoadmapItemCardProps) {
  const [expanded, setExpanded] = useState(false);
  const detailId = useId();

  return (
    <article className="roadmap-card" data-expanded={expanded}>
      <button
        type="button"
        className="roadmap-card__summary"
        aria-expanded={expanded}
        aria-controls={detailId}
        onClick={() => setExpanded((value) => !value)}
      >
        <span className="roadmap-card__title">{title}</span>
        <span className="roadmap-card__meta">
          <StatusPill status={status} />
          <span className="roadmap-card__chevron" aria-hidden="true">
            ⌄
          </span>
        </span>
      </button>

      {expanded ? (
        <div className="roadmap-card__detail" id={detailId}>
          <div className="metadata muted">
            <span>{voteCount} votes</span>
            <span>{commentCount} comments</span>
          </div>
          <Link className="roadmap-card__link" href={href}>
            View request →
          </Link>
        </div>
      ) : null}
    </article>
  );
}
