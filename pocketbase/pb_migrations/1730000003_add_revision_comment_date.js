// Add comment and created_at to bom_revisions for revision change notes.
// Run after 1730000002_part_master_categories.js.

migrate((db) => {
  const dao = new Dao(db);
  try {
    const c = dao.findCollectionByNameOrId("bom_revisions");
    if (c && c.schema) {
      const schema = c.schema;
      const fields = schema.fields();
      const hasComment = fields.some((f) => f && f.name === "comment");
      const hasCreatedAt = fields.some((f) => f && f.name === "created_at");
      if (!hasComment) {
        schema.addField(
          new SchemaField({
            name: "comment",
            type: "text",
            required: false,
            id: "rev_comment_f1",
          })
        );
      }
      if (!hasCreatedAt) {
        schema.addField(
          new SchemaField({
            name: "created_at",
            type: "text",
            required: false,
            id: "rev_created_f2",
          })
        );
      }
      dao.saveCollection(c);
    }
  } catch (e) {
    // Collection may not exist yet
  }
}, (db) => {
  const dao = new Dao(db);
  try {
    const c = dao.findCollectionByNameOrId("bom_revisions");
    if (c && c.schema) {
      try {
        c.schema.removeField(c.schema.getFieldByName("comment").id);
      } catch {}
      try {
        c.schema.removeField(c.schema.getFieldByName("created_at").id);
      } catch {}
      dao.saveCollection(c);
    }
  } catch (e) {}
});
