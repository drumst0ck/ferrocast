import { date, index, integer, pgEnum, pgTable, primaryKey, serial, text, timestamp, unique } from 'drizzle-orm/pg-core';

export const categoryEnum = pgEnum('entry_category', ['added', 'changed', 'fixed', 'removed', 'security', 'limitation']);

export const releases = pgTable('releases', {
  id: serial('id').primaryKey(),
  version: text('version').notNull().unique(),
  releasedOn: date('released_on', { mode: 'string' }),
  title: text('title'),
  summary: text('summary'),
  createdAt: timestamp('created_at', { withTimezone: true }).notNull().defaultNow(),
  updatedAt: timestamp('updated_at', { withTimezone: true }).notNull().defaultNow(),
});

export const entries = pgTable(
  'entries',
  {
    id: serial('id').primaryKey(),
    releaseId: integer('release_id').notNull().references(() => releases.id, { onDelete: 'cascade' }),
    category: categoryEnum('category').notNull(),
    slug: text('slug').notNull(),
    title: text('title'),
    bodyMd: text('body_md').notNull(),
    bodyHtml: text('body_html').notNull(),
    docsUrl: text('docs_url'),
    position: integer('position').notNull(),
    createdAt: timestamp('created_at', { withTimezone: true }).notNull().defaultNow(),
    updatedAt: timestamp('updated_at', { withTimezone: true }).notNull().defaultNow(),
  },
  (t) => [unique('entries_release_slug').on(t.releaseId, t.slug), index('entries_release_idx').on(t.releaseId, t.position)],
);

export const tags = pgTable('tags', {
  id: serial('id').primaryKey(),
  name: text('name').notNull().unique(),
});

export const entryTags = pgTable(
  'entry_tags',
  {
    entryId: integer('entry_id').notNull().references(() => entries.id, { onDelete: 'cascade' }),
    tagId: integer('tag_id').notNull().references(() => tags.id, { onDelete: 'cascade' }),
  },
  (t) => [primaryKey({ columns: [t.entryId, t.tagId] })],
);

export const imports = pgTable('imports', {
  id: serial('id').primaryKey(),
  sourceSha: text('source_sha').notNull(),
  importedAt: timestamp('imported_at', { withTimezone: true }).notNull().defaultNow(),
  entriesCount: integer('entries_count').notNull(),
});
