import { CodeBlock } from "@/components/docs/code-block";
import { Callout, Code, DocTitle, H2, Lead, P, Table, Ul, Li } from "@/components/docs/prose";

export default function Payments() {
  return (
    <article>
      <DocTitle ornament="❦ blueprint" planned>
        payments &amp; subscriptions
      </DocTitle>
      <Lead>
        billing as part of the application model: plans declared in the manifest,
        checkout in one call, entitlements checked where everything else is checked —
        in the signature. stripe rails and ethereum rails, one api.
      </Lead>

      <Callout tone="planned">
        this page is a design document. the API below is settled in shape but not yet
        implemented — names may drift before it ships.
      </Callout>

      <H2>configuration</H2>
      <P>
        plans are data, so they live in <Code>loop.toml</Code>. each plan can carry a
        stripe price, an eth price, or both — a plan with both lets the user choose
        the rail at checkout.
      </P>
      <CodeBlock
        language="toml"
        title="loop.toml"
        code={`[payments]

[payments.stripe]
secret = "env:STRIPE_SECRET"
webhook_secret = "env:STRIPE_WEBHOOK_SECRET"

[payments.eth]
receiver = "0x7f3a…c2e1"
confirmations = 3

[payments.plans.pro]
title = "pro"
stripe_price = "price_1QxT…"
eth_price = "0.005/month"

[payments.plans.studio]
title = "studio"
stripe_price = "price_1QxU…"
eth_price = "0.02/month"`}
      />
      <P>
        secrets use <Code>env:</Code> references — the manifest names the variable,
        the value stays in the environment. enabling payments mounts the stripe
        webhook route and starts the eth confirmation watcher.
      </P>

      <H2>checkout</H2>
      <P>
        one call creates a checkout regardless of rail. stripe yields a hosted
        checkout url; eth yields an invoice — a payment address, an exact amount, and
        an expiry — that the watcher resolves once enough confirmations land.
      </P>
      <CodeBlock
        title="planned — starting a subscription"
        code={`use lib::auth::User;
use lib::pay;

#[rest(post, "/subscribe")]
fn subscribe(user: User, plan: String, rail: Rail) -> Result<Checkout, HandlerError> {
    Ok(pay::checkout(&user, &plan).via(rail)?)
}

pub enum Rail {
    Stripe,
    Eth,
}

pub enum Checkout {
    Redirect { url: String },
    Invoice { address: Address, amount: Wei, expires: Date },
}`}
      />

      <H2>entitlements</H2>
      <P>
        the guard is the signature, same as auth: a <Code>Subscriber</Code> parameter
        requires an active subscription and answers 402 without one. gating on a
        specific plan is a runtime check with the same error path.
      </P>
      <CodeBlock
        title="planned — checking access"
        code={`use lib::pay::{self, Subscriber};

#[rest(get, "/render")]
fn render(sub: Subscriber, scene: Scene) -> Result<Blob, HandlerError> {
    let sub = pay::require(&sub, "studio")?;
    Ok(render_scene(scene, sub.seats()))
}

#[rest(get, "/billing")]
fn billing(user: User) -> BillingView {
    match pay::subscription(&user) {
        Some(sub) => BillingView::active(sub.plan(), sub.renews_at()),
        None => BillingView::free(),
    }
}`}
      />

      <H2>subscription state</H2>
      <Table
        head={["state", "meaning"]}
        rows={[
          ["active", "paid and current, entitlements on"],
          ["past_due", "renewal missed, grace period running"],
          ["canceled", "user ended it; active until the period closes"],
          ["expired", "entitlements off"],
        ]}
      />
      <P>
        state transitions arrive from stripe webhooks and from watched eth transfers,
        and both funnel into one stream your code can subscribe to — which composes
        directly with <Code>#[sse]</Code>:
      </P>
      <CodeBlock
        title="planned — billing events as a stream"
        code={`#[sse("/billing/events")]
fn billing_events(user: User) -> Result<impl Iterator<Item = PayEvent>, HandlerError> {
    Ok(pay::events().for_user(&user))
}`}
      />

      <H2>one-off charges</H2>
      <Ul>
        <Li>
          <Code>pay::charge(&amp;user, Cents(499))</Code> — a stripe payment intent
          against the customer&apos;s saved method
        </Li>
        <Li>
          <Code>pay::invoice(Wei::from_eth(0.01))</Code> — a watched eth invoice, no
          account required
        </Li>
        <Li>
          both return a <Code>Payment</Code> whose settlement can be awaited or
          observed on the event stream
        </Li>
      </Ul>

      <Callout>
        eth subscriptions are pull-less — there is no on-chain mandate. the watcher
        treats a subscription as an invoice that recurs: pay by the renewal date or
        the state machine walks the same past_due → expired path stripe renewals do.
      </Callout>
    </article>
  );
}
