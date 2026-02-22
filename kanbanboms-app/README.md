# Kanban BOMs (egui/web)

A kanban-style BOM kitting app built with Rust, egui, and eframe. Runs as a native desktop app or in the browser via WebAssembly.

## Features

- **BOM Search**: Search assemblies by part number or description (keyword AND filter)
- **Request Editing**: Add assemblies to a kitting request, edit quantities, clear list
- **PDF Print Preview**: Generate and download PDF of kitting BOM (header, assemblies table, parts table)
- **CSV Export**: Export aggregated parts list as CSV
- **BOM Editor**: Edit BOM structure (add/remove components, set quantities, disable entries)
- **Import**: Paste CSV (populate_db format) or JSON to load BOM data
- **Persistence**: Data saved to localStorage (web) / eframe storage (native)

## Build & Run

### Native

```bash
cargo run --release
```

### Web (WASM)

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk
trunk serve
```

Then open http://127.0.0.1:8080 (append `#dev` to skip service worker cache during development).

### Build for production

```bash
trunk build --release
```

Output in `dist/` - deploy to any static host (GitHub Pages, Netlify, etc.).

## CSV Import Format

Matches the Django `populate_db` format. Columns:
- Row 1: assembly partno (col 1), description (col 2)
- Row 9: component partno, col 10: description, col 6: quantity, col 8: disabled (yes/no)

## Data Model

- **BOM**: partno, description, batch_quantity, location
- **BOMEntry**: assembly → component, quantity, disabled
- **Request**: requested_by, machine_number, notes
- **RequestEntry**: request → assembly, quantity
