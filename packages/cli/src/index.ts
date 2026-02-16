#!/usr/bin/env node

import { existsSync } from 'node:fs';
import { parseArgs } from 'node:util';
import { resolve } from 'node:path';
import { startDevServer } from './dev.js';
import { buildPdf } from './build.js';

const USAGE = `
forme - Page-native PDF rendering engine

Usage:
  forme dev <file.tsx>    Start dev server with live preview
  forme build <file.tsx>  Render PDF to disk

Options:
  -o, --output <path>     Output PDF path (build only, default: output.pdf)
  -d, --data <path>       JSON data file to pass to template function
  -p, --port <number>     Dev server port (default: 4242)
  -h, --help              Show this help message

Examples:
  forme dev src/invoice.tsx
  forme build src/invoice.tsx -o invoice.pdf
  forme build src/report.tsx --data data.json -o report.pdf

Data flag:
  If your template exports a function instead of a JSX element,
  use --data to pass a JSON file as the function argument:

    // report.tsx
    export default function Report(data: { title: string }) {
      return <Document><Text>{data.title}</Text></Document>
    }

    forme build report.tsx --data '{"title": "Q4 Report"}'
`;

function main() {
  const { values, positionals } = parseArgs({
    allowPositionals: true,
    options: {
      output: { type: 'string', short: 'o', default: 'output.pdf' },
      data: { type: 'string', short: 'd' },
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

  // Validate input file exists
  const absoluteInput = resolve(inputPath);
  if (!existsSync(absoluteInput)) {
    console.error(`Error: Input file not found: ${absoluteInput}`);
    process.exit(1);
  }

  // Validate data file exists if provided
  const dataPath = values.data;
  if (dataPath) {
    const absoluteData = resolve(dataPath);
    if (!existsSync(absoluteData)) {
      console.error(`Error: Data file not found: ${absoluteData}`);
      process.exit(1);
    }
  }

  switch (command) {
    case 'dev':
      startDevServer(inputPath, { port: Number(values.port), dataPath });
      break;
    case 'build':
      buildPdf(inputPath, { output: values.output!, dataPath });
      break;
    default:
      console.error(`Unknown command: ${command}\n`);
      console.log(USAGE.trim());
      process.exit(1);
  }
}

main();
