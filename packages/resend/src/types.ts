import type { ReactElement } from 'react';

interface BaseOptions {
  resendApiKey: string;
  from: string;
  to: string | string[];
  subject: string;
  filename?: string;
  html?: string;
  text?: string;
  react?: ReactElement;
  cc?: string | string[];
  bcc?: string | string[];
  replyTo?: string;
  tags?: { name: string; value: string }[];
  headers?: Record<string, string>;
}

interface PdfBytesOptions extends BaseOptions {
  pdf: Uint8Array;
  template?: never;
  data?: never;
  render?: never;
}

interface TemplateOptions extends BaseOptions {
  pdf?: never;
  template: string;
  data?: Record<string, any>;
  render?: never;
}

interface RenderOptions extends BaseOptions {
  pdf?: never;
  template?: never;
  data?: never;
  render: () => ReactElement;
}

export type SendPdfOptions = PdfBytesOptions | TemplateOptions | RenderOptions;

export interface RenderAttachOptions {
  template?: string;
  data?: Record<string, any>;
  render?: () => ReactElement;
  filename?: string;
}
