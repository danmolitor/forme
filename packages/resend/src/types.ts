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

interface TemplateOptions extends BaseOptions {
  template: string;
  data?: Record<string, any>;
  render?: never;
}

interface RenderOptions extends BaseOptions {
  template?: never;
  data?: never;
  render: () => ReactElement;
}

export type SendPdfOptions = TemplateOptions | RenderOptions;

export interface RenderAttachOptions {
  template?: string;
  data?: Record<string, any>;
  render?: () => ReactElement;
  filename?: string;
}
