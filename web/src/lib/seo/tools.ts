import toolsData from '../../data/tools.json';
import { levels, toolLevel, type Level } from '../../data/status';

export interface ToolPage {
  id: string;
  slug: string;
  name: string;
  docSlug: string;
  status: string;
  level: Level;
  shortcut: string | null;
  formats: string[];
  screenshot: string | null;
  media: 'image' | 'video' | 'audio' | 'pdf' | 'archive' | 'any';
}

// id in tools.json -> [docs file name, screenshot in public/img, media]
const MAP: Record<string, [string, string | null, ToolPage['media']]> = {
  compress: ['compress', 'files-compress-image', 'any'],
  removeMetadata: ['edit-metadata', 'files-meta', 'any'],
  editImage: ['edit-photos', 'files-edit', 'image'],
  annotateImage: ['annotate', 'files-annotate', 'image'],
  frameImage: ['add-background', 'files-frame', 'image'],
  cropImage: ['crop-image', 'files-crop', 'image'],
  redactImage: ['redact-image', 'files-redact', 'image'],
  createCollage: ['collage', 'files-collage', 'image'],
  trimVideo: ['trim-video', 'files-vtrim', 'video'],
  cropVideo: ['crop-video', 'files-vcrop', 'video'],
  changeVideoSpeed: ['video-speed', 'files-vspeed', 'video'],
  joinVideos: ['join-videos', 'files-reorder', 'video'],
  videoSnapshots: ['video-snapshots', 'files-vsnapshot', 'video'],
  splitVideo: ['split-video', 'files-vsplit', 'video'],
  redactVideo: ['redact-video', 'files-vredact', 'video'],
  muteVideo: ['mute-video', null, 'video'],
  normalizeAudio: ['normalize-audio', 'files-anorm', 'audio'],
  audioToVideo: ['audio-to-video', 'files-avis', 'audio'],
  trimAudio: ['trim-audio', 'files-atrim', 'audio'],
  audioChannels: ['audio-channels', 'files-achan', 'audio'],
  redactAudio: ['bleep-audio', 'files-bleep', 'audio'],
  mergePdf: ['merge-pdf', 'files-reorder', 'pdf'],
  organizePdf: ['organize-pdf', 'files-organize', 'pdf'],
  splitPdf: ['split-pdf', null, 'pdf'],
  createPdf: ['create-pdf', 'files-reorder', 'image'],
  extractArchive: ['extract-archive', null, 'archive'],
};

export function allTools(): ToolPage[] {
  return toolsData.tools.map((t) => {
    const m = MAP[t.id];
    if (!m) throw new Error(`no tool page mapping for "${t.id}"`);
    return {
      id: t.id,
      slug: m[0],
      name: t.name,
      docSlug: m[0],
      status: t.status,
      level: toolLevel(t.status),
      shortcut: t.shortcut,
      formats: t.formats,
      screenshot: m[1] ? `/img/${m[1]}.jpg` : null,
      media: m[2],
    };
  });
}

export function toolMeta(t: ToolPage): { title: string; description: string } {
  const title = `${t.name} on Mac, offline · Ferrocast`;
  const kind = t.status === 'window' ? 'a window' : 'a one-step preset';
  const opts = [
    `${t.name} on your Mac with Ferrocast: ${kind}, ${levels[t.level].label.toLowerCase()} status, local only, nothing uploaded. Open it from the Tools wheel (⌘ while dragging).`,
    `${t.name} on your Mac with Ferrocast: ${kind}, ${levels[t.level].label.toLowerCase()} status, local only, nothing uploaded. Open it from the Tools wheel.`,
    `${t.name} on your Mac with Ferrocast: ${kind}, local only, nothing uploaded. Open it from the Tools wheel.`,
  ];
  const description = opts.find((o) => o.length <= 155) ?? opts[opts.length - 1];
  return { title, description };
}
