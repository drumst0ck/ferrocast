/*
 * Copy for /raycast-script-commands/ (English, per the Notion board idea "Run Raycast Script Commands
 * without Raycast"). Facts come from crates/core/src/script.rs (metadata parser, refresh rules,
 * ANSI stripping), crates/app/src/services.rs (load_scripts, run_script, default folder) and
 * web/src/content/docs/docs/launcher/script-commands.md. Rules: never claim full Raycast
 * compatibility. Extension support is partial and opt-in. No statement about Raycast
 * beyond its public repository and the v0.4.0 release notes.
 */
import type { GuideCopy } from './compress-video';

export const page: GuideCopy = {
  lang: 'en',
  path: '/raycast-script-commands/',
  title: 'Run Raycast Script Commands without Raycast on Mac | Ferrocast',
  description:
    'Ferrocast runs Raycast Script Commands as they are: same @raycast metadata, full, compact, silent and inline modes, up to three arguments. Free and open source.',
  crumbs: [['Home', '/'], ['Raycast Script Commands', '/raycast-script-commands/']],
  h1: 'Run your Raycast Script Commands without Raycast',
  lead: [
    'Ferrocast reads the same `@raycast.*` comments that Raycast Script Commands use, so a script you wrote for Raycast, or one from the community [raycast/script-commands](https://github.com/raycast/script-commands) repository, shows up in the Ferrocast launcher without changes. Put it in a scripts folder and type its title.',
    'Ferrocast is a free, open-source (MIT) launcher for Apple Silicon Macs. It runs Script Commands. It can also run extensions you already installed with Raycast, off by default and not in full. It does not contact Raycast’s Store.',
  ],
  cta: { dmg: 'Download Ferrocast', note: 'Free · MIT · Apple Silicon · no account' },
  limits: {
    h: 'Which metadata Ferrocast reads',
    caption: 'Raycast Script Command metadata keys and what Ferrocast does with each',
    cols: ['Key', 'What it does in Ferrocast', 'Required'],
    rows: [
      ['`schemaVersion`', 'Must be `1`; other values skip the script.', 'Yes'],
      ['`title`', 'The name you type in the launcher.', 'Yes'],
      ['`mode`', '`fullOutput`, `compact`, `silent` or `inline` (see below). Any other value skips the script.', 'Yes'],
      ['`icon`', 'An emoji icon is shown next to the command; icons that are image files are not.', 'No'],
      ['`packageName`', 'Shown under the title in the launcher.', 'No'],
      ['`description`', 'Its words also find the command when you search.', 'No'],
      ['`argument1` to `argument3`', 'Up to three arguments of type `text`, `password` or `dropdown`, with `placeholder`, `optional` and `percentEncoded`.', 'No'],
      ['`refreshTime`', 'How often an `inline` command reruns: `10s`, `1m`, `1h` or `1d`, at least 10 seconds.', 'For inline'],
      ['`currentDirectoryPath`', 'Where the script runs: an absolute path, a path starting with `~`, or one relative to the script. Default: the script’s folder.', 'No'],
      ['`needsConfirmation`', '`true` asks before running.', 'No'],
    ],
    note: 'From Ferrocast’s metadata parser. A script with a fourth argument still loads; only the first three are used.',
  },
  steps: {
    h: 'How to use your scripts in Ferrocast',
    howToName: 'Run Raycast Script Commands in Ferrocast',
    items: [
      ['Install Ferrocast', 'Download the DMG or install it with Homebrew.'],
      ['Add your scripts folder', 'In Settings → Scripts, add the folder that holds your Script Commands. Ferrocast also has its own folder, ~/Library/Application Support/Ferrocast/scripts, with two examples.'],
      ['Make sure each script can run', 'Ferrocast runs the file directly, so it needs a shebang line (such as #!/bin/bash) and execute permission (chmod +x).'],
      ['Type the title', 'Open the launcher with ⌥ Space and type the script’s title. Arguments are asked for in the launcher.'],
    ],
    code: `#!/bin/bash
# @raycast.schemaVersion 1
# @raycast.title Uptime
# @raycast.mode compact
# @raycast.packageName Examples

uptime | sed 's/.*up/up/'`,
  },
  sections: [
    {
      h: 'The four output modes',
      paras: [],
      list: [
        '**fullOutput**: shows everything the script prints in a view you can copy from.',
        '**compact**: shows the last line of output as a short message.',
        '**silent**: runs without showing output.',
        '**inline**: shows the first line of output in the launcher and reruns on its `refreshTime`. Without `refreshTime`, Ferrocast treats it as `compact`.',
      ],
    },
    {
      h: 'Create a new Script Command',
      paras: ['Run **Create Script Command** from the launcher. Ferrocast writes a template with the required metadata into its scripts folder and makes it executable, so you only add the code.'],
    },
    {
      h: 'Limits',
      paras: [],
      list: [
        'Arguments, including `password` ones, are passed on the command line, as in Raycast, and can be seen with `ps` while the script runs. Do not pass critical secrets this way.',
        'Colour codes (ANSI) are removed from the output.',
        'Up to three arguments are used.',
        'Extensions you already installed with Raycast can run when you turn that on in Settings → Integrations. Ferrocast does not contact Raycast’s Store and has no store of its own. Menu bar commands, OAuth sign-in, AI features, the file picker and the link accessory are not supported yet. See [Run Raycast extensions](/docs/launcher/raycast-extensions/).',
        'Apple Silicon Macs only.',
      ],
    },
  ],
  faq: {
    h: 'Questions',
    items: [
      ['Can I run Raycast Script Commands without Raycast?', 'Yes. Ferrocast reads the same @raycast metadata and runs Script Commands in its launcher, with the fullOutput, compact, silent and inline modes.'],
      ['Do I have to change my scripts?', 'Usually not. A script needs schemaVersion 1, a title and a mode, a shebang line and execute permission. Inline scripts also need refreshTime; without it they run as compact.'],
      ['Do Raycast extensions work in Ferrocast?', 'Partly. Ferrocast runs extensions you already installed with Raycast, from ~/.config/raycast/extensions, when you turn that on. It needs Node.js 22 or newer. It does not contact Raycast’s Store and has no store of its own. In a test, 34 of 40 view commands from 20 popular extensions worked. Menu bar commands, OAuth sign-in, AI features, the file picker and the link accessory are not supported yet. Script Commands are separate and run as they are.'],
      ['Where do I put my scripts?', 'In any folder you add in Settings → Scripts, or in Ferrocast’s own folder at ~/Library/Application Support/Ferrocast/scripts.'],
      ['Are script arguments private?', 'Not fully. Like in Raycast, arguments are passed on the command line and can be seen with ps while the script runs.'],
      ['Is Ferrocast free?', 'Yes. Ferrocast is free and open source under the MIT licence, with no account and no paid plan.'],
    ],
  },
  more: {
    h: 'Related',
    links: [
      ['Script Commands docs', '/docs/launcher/script-commands/'],
      ['Run Raycast extensions', '/docs/launcher/raycast-extensions/'],
      ['Migrating from Raycast', '/docs/getting-started/migrating-from-raycast/'],
      ['A free Raycast alternative', '/raycast-alternative/'],
      ['Settings', '/docs/launcher/settings/'],
    ],
  },
};
