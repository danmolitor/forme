export interface Theme {
  primaryColor?: string;
  fontFamily?: string;
  margins?: number | { top: number; right: number; bottom: number; left: number };
}

export interface InvoiceData {
  invoiceNumber: string;
  date: string;
  dueDate: string;
  taxRate: number;
  company: {
    name: string;
    initials: string;
    address: string;
    cityStateZip: string;
    email: string;
    logoUrl?: string;
  };
  billTo: {
    name: string;
    company: string;
    address: string;
    cityStateZip: string;
    email: string;
  };
  shipTo: {
    name: string;
    address: string;
    cityStateZip: string;
  };
  items: {
    description: string;
    quantity: number;
    unitPrice: number;
    /** Line discount as a decimal, e.g. 0.1 for 10%. Matches `taxRate`'s units. */
    discount?: number;
  }[];
  paymentTerms: string;
  notes?: string;
  theme?: Theme;
}

export interface ReceiptData {
  receiptNumber: string;
  date: string;
  taxRate: number;
  store: {
    name: string;
    address: string;
    cityStateZip: string;
    phone: string;
    website: string;
  };
  items: {
    name: string;
    price: number;
    quantity?: number;
  }[];
  paymentMethod: string;
  cardLastFour?: string;
  theme?: Theme;
}

export interface ReportData {
  title: string;
  subtitle: string;
  author: string;
  department: string;
  company: string;
  date: string;
  classification: string;
  keyMetrics?: { value: string; label: string }[];
  sections: {
    title: string;
    paragraphs?: string[];
    intro?: string;
    tableData?: { region: string; q1: string; q2: string; q3: string; q4: string }[];
    items?: { title: string; description: string; priority: string; timeline: string }[];
  }[];
  theme?: Theme;
}

export interface ShippingLabelData {
  tracking: string;
  trackingUrl?: string;
  service: string;
  weight: string;
  dimensions: string;
  from: {
    name: string;
    address: string;
    cityStateZip: string;
  };
  to: {
    name: string;
    address: string;
    address2?: string;
    cityStateZip: string;
  };
  stamps?: string[];
  theme?: Theme;
}

export interface LetterData {
  sender: {
    name: string;
    title?: string;
    company: string;
    address: string;
    cityStateZip: string;
    phone?: string;
    email?: string;
    logoUrl?: string;
  };
  date: string;
  recipient: {
    name: string;
    title?: string;
    company?: string;
    address: string;
    cityStateZip: string;
  };
  salutation: string;
  body: string[];
  closing: string;
  signatureName: string;
  signatureTitle?: string;
  theme?: Theme;
}
