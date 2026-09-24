# Development Roadmap & Milestone Report: File Inspector Enhancements - 24-Sep-2026

This document outlines the structured todos, implementation phases, and core architectural design decisions for upgrading the Rust file inspector application.

---

## 📌 Scope & Exclusions Overview

### Active Focus Areas

1. **Handling File Moves & Renames Gracefully** (Content hashing & orphan record re-linking)
2. **Improving Search User Experience (UX)** (Fuzzy search, debounced async search, and faceted click-pills)
3. **Enhancing General App Usability** (Viewport virtualization, persistent UI configuration, and background status toasts)

### Deferred Scope (Explicitly Excluded)

* ❌ **File System Watchers** (`notify` crate integration) — Deferred to a future release cycle.
* ❌ **Duplicate File Handling** (Multi-stage hashing pipeline & deduplication UI) — Out of scope for current milestones.

---

## 🛠️ Phase-by-Phase Todos & Design Decisions

### Milestone 1: Resilient Identity & Relocation Management

*Goal: Prevent broken metadata links when files are renamed or moved outside the application interface.*

* **[ ] Task 1.1: Decouple Identity from Path**
* **Todo:** Update SQLite schema logic so that operations query primarily by file hash (`file_hash`) rather than absolute `path`.
* **Design Decision:** The content hash (SHA-256) serves as the permanent primary key. Paths are treated as mutable metadata properties attached to a content hash.


* **[ ] Task 1.2: Implement Orphan Record Re-linking (Move Detection)**
* **Todo:** During directory scans or validation passes, check if an unresolving path corresponds to an existing `file_hash` in the database.
* **Design Decision:** Rather than deleting the record and creating a duplicate entry, automatically update the `path` column for the matching hash. This preserves user-defined custom tags and creation histories seamlessly.



---

### Milestone 2: Modernized Search & Discovery Experience

*Goal: Transition from rigid substring matching to an agile, fluid, and responsive discovery workflow.*

* **[ ] Task 2.1: Fuzzy Search Integration**
* **Todo:** Replace or augment current text containment checks with a fuzzy-matching library (such as `fuzzy-matcher` or `nucleo`).
* **Design Decision:** Allow minor user typos and non-contiguous character sequences to match filenames and tags effectively.


* **[ ] Task 2.2: Debounced Asynchronous Search**
* **Todo:** Decouple query filtering from the immediate main render frame by introducing a background thread channel or debounce timer.
* **Design Decision:** Prevents UI stuttering and input lag when typing queries in large databases containing tens of thousands of indexed records.


* **[ ] Task 2.3: Faceted Search & Clickable Pills**
* **Todo:** Transform the tag statistics and metadata attributes into interactive filtering chips/pills in the UI.
* **Design Decision:** Clicking any tag, extension, year, or month instantly toggles an additive filter state, enabling rapid drill-down searches without manual syntax typing.



---

### Milestone 3: Performance & Usability Overhaul

*Goal: Guarantee smooth UI rendering under heavy loads and preserve workspace state across application relaunches.*

* **[ ] Task 3.1: Viewport Virtualization for Large Result Sets**
* **Todo:** Replace standard dense loops in `egui::ScrollArea` with `egui_extras::TableBuilder` or list virtualization.
* **Design Decision:** Only render DOM/UI primitives for elements currently visible within the active viewport, keeping frame rates locked at 60 FPS even with thousands of search hits.


* **[ ] Task 3.2: Persistent UI State Management**
* **Todo:** Serialize layout configurations—such as window dimensions, panel widths, and recent search history—directly into `config.json`.
* **Design Decision:** Provide a continuous user experience by restoring the exact workspace layout upon application relaunch.


* **[ ] Task 3.3: Background Status Toasts**
* **Todo:** Replace static status message labels with transient, auto-fading notification toasts.
* **Design Decision:** Provide non-intrusive visual feedback for background actions like "Tags saved successfully" or "Export completed."



---

## 📊 Summary Milestone Report

| Milestone | Target Component | Core Impact | Status |
| --- | --- | --- | --- |
| **Phase 1** | Relocation & Identity | Preserves tags and attributes during external file moves. | *Ready for Development* |
| **Phase 2** | Search UX & Filtering | Streamlines discovery via fuzzy matching and click-pills. | *Ready for Development* |
| **Phase 3** | Usability & Performance | Eliminates frame drops and retains layout continuity. | *Ready for Development* |