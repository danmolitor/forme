export function buildDefaultEmail(
  template: string | undefined,
  data: Record<string, any> | undefined
): { html: string; text: string } {
  const d = data || {};

  if (template === 'invoice') {
    const customerName = d.customer || 'there';
    const invoiceNum = d.invoiceNumber || '';
    const amount = d.total || '';
    const date = d.date || '';
    const company = d.company || '';

    const lines = [
      `Hi ${customerName},`,
      '',
      'Please find your invoice attached.',
      '',
      ...(invoiceNum ? [`Invoice: #${invoiceNum}`] : []),
      ...(amount ? [`Amount: ${amount}`] : []),
      ...(date ? [`Date: ${date}`] : []),
      '',
      'Best regards,',
      company,
    ];

    const text = lines.join('\n');
    const html = lines.map(l => l === '' ? '<br>' : `<p>${l}</p>`).join('\n');
    return { html, text };
  }

  if (template === 'receipt') {
    const customerName = d.customer || 'there';
    const amount = d.total || '';
    const date = d.date || '';
    const company = d.company || '';

    const lines = [
      `Hi ${customerName},`,
      '',
      'Thank you for your payment. Your receipt is attached.',
      '',
      ...(amount ? [`Amount: ${amount}`] : []),
      ...(date ? [`Date: ${date}`] : []),
      '',
      'Best regards,',
      company,
    ];

    const text = lines.join('\n');
    const html = lines.map(l => l === '' ? '<br>' : `<p>${l}</p>`).join('\n');
    return { html, text };
  }

  return {
    html: '<p>Your document is attached.</p>',
    text: 'Your document is attached.',
  };
}
