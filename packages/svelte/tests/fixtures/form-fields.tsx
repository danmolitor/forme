import { Document, Page, View, Text, TextField, Checkbox, Dropdown, RadioButton } from '@formepdf/react';

export interface Props {
  plan?: string;
}

/** TSX twin of form-fields.svelte for cross-adapter parity tests. */
export default function FormFieldsFixture({ plan = 'pro' }: Props) {
  return (
    <Document title="Registration Form">
      <Page size="A4" margin={40}>
        <Text style={{ fontSize: 20, marginBottom: 16 }}>Registration</Text>

        <Text>Full name</Text>
        <TextField name="full_name" width={220} placeholder="Jane Doe" maxLength={64} />

        <Text style={{ marginTop: 12 }}>Bio</Text>
        <TextField
          name="bio"
          width={400}
          height={80}
          multiline
          fontSize={10}
          value="Hello!"
          style={{ marginBottom: 8 }}
        />

        <TextField name="access_code" width={180} password readOnly value="s3cret" />

        <View style={{ flexDirection: 'row', gap: 8, marginTop: 12 }}>
          <Checkbox name="agree_terms" checked />
          <Text>I agree to the terms</Text>
        </View>
        <View style={{ flexDirection: 'row', gap: 8 }}>
          <Checkbox name="newsletter" width={18} height={18} readOnly />
          <Text>Subscribe to the newsletter</Text>
        </View>

        <Text style={{ marginTop: 12 }}>Country</Text>
        <Dropdown name="country" options={['US', 'UK', 'CA']} width={200} value="UK" fontSize={11} />
        <Dropdown
          name="plan_type"
          options={['Free', 'Pro', 'Team']}
          width={160}
          height={28}
          readOnly
          style={{ marginTop: 8 }}
        />

        <Text style={{ marginTop: 12 }}>Plan</Text>
        <View style={{ flexDirection: 'row', gap: 16 }}>
          <RadioButton name="plan" value="free" checked={plan === 'free'} />
          <RadioButton name="plan" value="pro" checked={plan === 'pro'} />
          <RadioButton name="plan" value="team" checked={plan === 'team'} width={16} height={16} />
        </View>
      </Page>
    </Document>
  );
}
