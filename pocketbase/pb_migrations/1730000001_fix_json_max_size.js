// Fix JSON field maxSize for collections created before maxSize was set.
// Run after 1730000000_kanbanboms_collections.js.
// Without this, custom_fields/tags/entries fail with "maximum allowed JSON size is 0 bytes".

migrate((db) => {
  const dao = new Dao(db);
  const collections = ["boms", "bom_entries", "bom_revisions"];
  for (const name of collections) {
    try {
      const c = dao.findCollectionByNameOrId(name);
      const schema = c.schema;
      if (schema) {
        const fields = schema.fields();
        for (let i = 0; i < fields.length; i++) {
          const f = fields[i];
          if (f && f.type === "json") {
            f.options = f.options || {};
            f.options.maxSize = 2000000;
          }
        }
      }
      dao.saveCollection(c);
    } catch (e) {
      // Collection may not exist yet (fresh install with updated 1730000000)
    }
  }
}, (db) => {
  // No revert - maxSize increase is safe to keep
});
