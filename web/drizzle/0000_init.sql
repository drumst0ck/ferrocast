CREATE TYPE "public"."entry_category" AS ENUM('added', 'changed', 'fixed', 'removed', 'security', 'limitation');--> statement-breakpoint
CREATE TABLE "entries" (
	"id" serial PRIMARY KEY NOT NULL,
	"release_id" integer NOT NULL,
	"category" "entry_category" NOT NULL,
	"slug" text NOT NULL,
	"title" text,
	"body_md" text NOT NULL,
	"body_html" text NOT NULL,
	"docs_url" text,
	"position" integer NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL,
	"updated_at" timestamp with time zone DEFAULT now() NOT NULL,
	CONSTRAINT "entries_release_slug" UNIQUE("release_id","slug")
);
--> statement-breakpoint
CREATE TABLE "entry_tags" (
	"entry_id" integer NOT NULL,
	"tag_id" integer NOT NULL,
	CONSTRAINT "entry_tags_entry_id_tag_id_pk" PRIMARY KEY("entry_id","tag_id")
);
--> statement-breakpoint
CREATE TABLE "imports" (
	"id" serial PRIMARY KEY NOT NULL,
	"source_sha" text NOT NULL,
	"imported_at" timestamp with time zone DEFAULT now() NOT NULL,
	"entries_count" integer NOT NULL
);
--> statement-breakpoint
CREATE TABLE "releases" (
	"id" serial PRIMARY KEY NOT NULL,
	"version" text NOT NULL,
	"released_on" date,
	"title" text,
	"summary" text,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL,
	"updated_at" timestamp with time zone DEFAULT now() NOT NULL,
	CONSTRAINT "releases_version_unique" UNIQUE("version")
);
--> statement-breakpoint
CREATE TABLE "tags" (
	"id" serial PRIMARY KEY NOT NULL,
	"name" text NOT NULL,
	CONSTRAINT "tags_name_unique" UNIQUE("name")
);
--> statement-breakpoint
ALTER TABLE "entries" ADD CONSTRAINT "entries_release_id_releases_id_fk" FOREIGN KEY ("release_id") REFERENCES "public"."releases"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "entry_tags" ADD CONSTRAINT "entry_tags_entry_id_entries_id_fk" FOREIGN KEY ("entry_id") REFERENCES "public"."entries"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "entry_tags" ADD CONSTRAINT "entry_tags_tag_id_tags_id_fk" FOREIGN KEY ("tag_id") REFERENCES "public"."tags"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
CREATE INDEX "entries_release_idx" ON "entries" USING btree ("release_id","position");