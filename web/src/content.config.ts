import { defineCollection } from 'astro:content';
import { docsLoader } from '@astrojs/starlight/loaders';
import { docsSchema } from '@astrojs/starlight/schema';
import { z } from 'astro/zod';

/**
 * Every docs page needs a description of 120-155 characters (used for search results and meta tags).
 */
export const collections = {
  docs: defineCollection({
    loader: docsLoader(),
    schema: docsSchema({
      extend: z.object({
        description: z.string().min(120, 'description must be 120-155 characters').max(155, 'description must be 120-155 characters'),
      }),
    }),
  }),
};
