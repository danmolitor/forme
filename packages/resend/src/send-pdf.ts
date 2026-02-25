import { Resend } from 'resend';
import { renderDocument } from '@formepdf/core';
import { getTemplate } from './templates/index.js';
import { buildDefaultEmail } from './default-email.js';
import type { SendPdfOptions } from './types.js';

export async function sendPdf(options: SendPdfOptions): Promise<{ id: string }> {
  const {
    resendApiKey, from, to, subject,
    template, data, render, filename,
    html, text, react,
    cc, bcc, replyTo, tags, headers,
  } = options;

  let element;
  if (render) {
    element = render();
  } else if (template) {
    const templateFn = getTemplate(template);
    if (!templateFn) {
      throw new Error(`Unknown template: "${template}". Use listTemplates() to see available templates.`);
    }
    element = templateFn(data || {});
  } else {
    throw new Error('Either "template" or "render" must be provided.');
  }

  const pdfBytes = await renderDocument(element);
  const pdfFilename = filename || `${template || 'document'}.pdf`;
  const attachment = {
    filename: pdfFilename,
    content: Buffer.from(pdfBytes),
  };

  let emailHtml = html;
  let emailText = text;
  if (!html && !text && !react) {
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

  const { data: result, error } = await resend.emails.send(emailPayload as any);

  if (error) {
    throw new Error(`Resend error: ${error.message}`);
  }

  return { id: result!.id };
}
