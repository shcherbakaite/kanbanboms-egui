// KanbanBOMs collections for shared state backend.
// Run: ./pocketbase serve (from pocketbase/ directory)
// Migrations run automatically on first serve.
// Note: PocketBase 0.22 uses Dao; 0.23+ uses app.save() directly.

migrate((db) => {
  const dao = new Dao(db);

  // boms: parts/assemblies
  const boms = new Collection({
    type: "base",
    name: "boms",
    listRule: "",
    viewRule: "",
    createRule: "",
    updateRule: "",
    deleteRule: "",
    schema: [
      { name: "uuid", type: "text", required: true },
      { name: "partno", type: "text", required: true },
      { name: "description", type: "text", required: false },
      { name: "batch_quantity", type: "number", required: false },
      { name: "location", type: "text", required: false },
      { name: "custom_fields", type: "json", required: false, options: { maxSize: 2000000 } },
    ],
    indexes: ["CREATE UNIQUE INDEX idx_boms_uuid ON boms (uuid)"],
  });
  dao.saveCollection(boms);

  // bom_entries: BOM structure (assembly -> component)
  const bomEntries = new Collection({
    type: "base",
    name: "bom_entries",
    listRule: "",
    viewRule: "",
    createRule: "",
    updateRule: "",
    deleteRule: "",
    schema: [
      { name: "bom_uuid", type: "text", required: true },
      { name: "part_uuid", type: "text", required: true },
      { name: "quantity", type: "number", required: true },
      { name: "disabled", type: "bool", required: false },
      { name: "tags", type: "json", required: false, options: { maxSize: 2000000 } },
    ],
  });
  dao.saveCollection(bomEntries);

  // requests: kitting requests
  const requests = new Collection({
    type: "base",
    name: "requests",
    listRule: "",
    viewRule: "",
    createRule: "",
    updateRule: "",
    deleteRule: "",
    schema: [
      { name: "uuid", type: "text", required: true },
      { name: "requested_by", type: "text", required: false },
      { name: "machine_number", type: "text", required: false },
      { name: "notes", type: "text", required: false },
    ],
    indexes: ["CREATE UNIQUE INDEX idx_requests_uuid ON requests (uuid)"],
  });
  dao.saveCollection(requests);

  // request_entries: assemblies in a request
  const requestEntries = new Collection({
    type: "base",
    name: "request_entries",
    listRule: "",
    viewRule: "",
    createRule: "",
    updateRule: "",
    deleteRule: "",
    schema: [
      { name: "request_uuid", type: "text", required: true },
      { name: "part_uuid", type: "text", required: true },
      { name: "quantity", type: "number", required: true },
    ],
  });
  dao.saveCollection(requestEntries);

  // bom_revisions: BOM entry snapshots
  const bomRevisions = new Collection({
    type: "base",
    name: "bom_revisions",
    listRule: "",
    viewRule: "",
    createRule: "",
    updateRule: "",
    deleteRule: "",
    schema: [
      { name: "bom_uuid", type: "text", required: true },
      { name: "revision", type: "number", required: true },
      { name: "entries", type: "json", required: true, options: { maxSize: 2000000 } },
    ],
  });
  dao.saveCollection(bomRevisions);
}, (db) => {
  const dao = new Dao(db);
  const names = ["bom_revisions", "request_entries", "requests", "bom_entries", "boms"];
  for (const name of names) {
    try {
      const c = dao.findCollectionByNameOrId(name);
      dao.deleteCollection(c);
    } catch {}
  }
});
