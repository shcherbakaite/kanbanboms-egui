// Add part_master_categories collection for shared category tabs.
// Run after 1730000001_fix_json_max_size.js.

migrate((db) => {
  const dao = new Dao(db);
  const partMasterCategories = new Collection({
    type: "base",
    name: "part_master_categories",
    listRule: "",
    viewRule: "",
    createRule: "",
    updateRule: "",
    deleteRule: "",
    schema: [
      { name: "name", type: "text", required: true },
      { name: "sort_order", type: "number", required: true },
    ],
    indexes: ["CREATE UNIQUE INDEX idx_pmc_name ON part_master_categories (name)"],
  });
  dao.saveCollection(partMasterCategories);
}, (db) => {
  const dao = new Dao(db);
  try {
    const c = dao.findCollectionByNameOrId("part_master_categories");
    dao.deleteCollection(c);
  } catch {}
});
