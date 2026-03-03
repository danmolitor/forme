import { Resend } from 'resend';
import { renderDocument } from '@formepdf/core';
import { getTemplate } from './templates/index.js';
import { buildDefaultEmail } from './default-email.js';
import type { SendPdfOptions } from './types.js';

export async function sendPdf(options: SendPdfOptions) {
  const {
    resendApiKey, from, to, subject,
    filename, html, text, react,
    cc, bcc, replyTo, tags, headers,
  } = options;

  let pdfBytes: Uint8Array;
  if ('pdf' in options && options.pdf) {
    pdfBytes = options.pdf;
  } else if ('render' in options && options.render) {
    pdfBytes = await renderDocument(options.render());
  } else if ('template' in options && options.template) {
    const templateFn = getTemplate(options.template);
    if (!templateFn) {
      throw new Error(`Unknown template: "${options.template}". Use listTemplates() to see available templates.`);
    }
    pdfBytes = await renderDocument(templateFn(options.data || {}));
  } else {
    throw new Error('One of "pdf", "render", or "template" must be provided.');
  }
  const template = 'template' in options ? options.template : undefined;
  const pdfFilename = filename || `${template || 'document'}.pdf`;
  const attachment = {
    filename: pdfFilename,
    content: Buffer.from(pdfBytes),
  };

  let emailHtml = html;
  let emailText = text;
  if (!html && !text && !react) {
    const data = 'data' in options ? options.data : undefined;
    const defaultEmail = buildDefaultEmail(template, data);
    emailHtml = defaultEmail.html;
    emailText = defaultEmail.text;
  }

  const resend = new Resend(resendApiKey);

  const emailPayload: Record<string, unknown> = {
    from,
    to: Array.isArray(to) ? to : [to],
    subject,
    attachments: [attachment],
  };
  if (react) emailPayload.react = react;
  if (emailHtml) emailPayload.html = emailHtml;
  if (emailText) emailPayload.text = emailText;
  if (cc) emailPayload.cc = Array.isArray(cc) ? cc : [cc];
  if (bcc) emailPayload.bcc = Array.isArray(bcc) ? bcc : [bcc];
  if (replyTo) emailPayload.replyTo = replyTo;
  if (tags) emailPayload.tags = tags;
  if (headers) emailPayload.headers = headers;

  return resend.emails.send(emailPayload as any);
}
