#!/usr/bin/env node

import { parseArgs } from 'node:util';
import { startDevServer } from './dev.js';
import { buildPdf } from './build.js';

const USAGE = `
forme - Page-native PDF rendering engine

Usage:
  forme dev <file.tsx>    Start dev server with live preview
  forme build <file.tsx>  Render PDF to disk

Options:
  -o, --output <path>     Output PDF path (build only, default: output.pdf)
  -p, --port <number>     Dev server port (default: 4242)
  -h, --help              Show this help message
`;

function main() {
  const { values, positionals } = parseArgs({
    allowPositionals: true,
    options: {
      output: { type: 'string', short: 'o', default: 'output.pdf' },
      port: { type: 'string', short: 'p', default: '4242' },
      help: { type: 'boolean', short: 'h', default: false },
    },
  });

  if (values.help || positionals.length === 0) {
    console.log(USAGE.trim());
    process.exit(0);
  }

  const [command, inputPath] = positionals;

  if (!inputPath) {
    console.error(`Error: Missing input file.\n`);
    console.log(USAGE.trim());
    process.exit(1);
  }

  switch (command) {
    case 'dev':
      startDevServer(inputPath, { port: Number(values.port) });
      break;
    case 'build':
      buildPdf(inputPath, { output: values.output! });
      break;
    default:
      console.error(`Unknown command: ${command}\n`);
      console.log(USAGE.trim());
      process.exit(1);
  }
}

main();
