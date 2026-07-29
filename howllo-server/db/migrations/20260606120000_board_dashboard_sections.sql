-- Which dashboard sections a board shows, and in what the web app treats as
-- canonical order: progress, latest, top. NULL/absent means "all sections"
-- (back-compat). An empty array means "hide them all".
ALTER TABLE boards
    ADD COLUMN IF NOT EXISTS dashboard_sections TEXT[] NOT NULL DEFAULT '{progress,latest,top}';
