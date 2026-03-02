// Make bom_entries.quantity optional so PocketBase accepts 0 (treated as missing for required number).
// Run after 1730000003_add_revision_comment_date.js.

migrate((db) => {
  const dao = new Dao(db);
  try {
    const c = dao.findCollectionByNameOrId("bom_entries");
    if (c && c.schema) {
      const fields = c.schema.fields();
      for (let i = 0; i < fields.length; i++) {
        const f = fields[i];
        if (f && f.name === "quantity") {
          f.required = false;
          break;
        }
      }
      dao.saveCollection(c);
    }
  } catch (e) {
    // Collection may not exist yet
  }
}, (db) => {
  const dao = new Dao(db);
  try {
    const c = dao.findCollectionByNameOrId("bom_entries");
    if (c && c.schema) {
      const fields = c.schema.fields();
      for (let i = 0; i < fields.length; i++) {
        const f = fields[i];
        if (f && f.name === "quantity") {
          f.required = true;
          break;
        }
      }
      dao.saveCollection(c);
    }
  } catch (e) {}
});
