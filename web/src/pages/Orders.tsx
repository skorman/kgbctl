import { useAccounts, useOpenOrders } from "../api/hooks";
import { Card, ErrorBanner } from "../components/Common";
import { Loading } from "../components/Splash";
import { dateTime, money, num, price, titleCase } from "../lib/format";

export function Orders() {
  const orders = useOpenOrders();
  const accounts = useAccounts();
  const acct = (id: number) => accounts.data?.find((a) => a.id === id)?.external_id ?? `#${id}`;
  if (orders.isPending) return <Loading text="Loading orders…" />;
  if (orders.isError) return <ErrorBanner error={orders.error} />;
  return (
    <div>
      <div className="page-head"><div><h1>Open orders</h1><p>Working orders recorded by the last sync.</p></div></div>
      <Card>
        {orders.data.length === 0 ? <div className="empty">No open orders</div> : (
          <div className="table-wrap"><table className="data num">
            <thead><tr><th>Order</th><th className="left">Account</th><th className="left">Side</th><th className="left">Type</th><th className="left">TIF</th><th>Qty</th><th>Filled</th><th>Limit</th><th>Stop</th><th>Avg fill</th><th>Fees</th><th className="left">Status</th><th className="left">Placed</th></tr></thead>
            <tbody>
              {orders.data.map((o) => (
                <tr key={o.id}>
                  <td className="mono">{o.broker_order_id}</td>
                  <td className="left mono">{acct(o.account_id)}</td>
                  <td className="left"><span className={`pill ${o.side === "buy" ? "good" : "bad"}`}>{o.side.toUpperCase()}</span></td>
                  <td className="left">{titleCase(o.order_type)}</td>
                  <td className="left muted">{o.time_in_force ?? "—"}</td>
                  <td>{num(o.quantity, 4)}</td>
                  <td>{num(o.filled_quantity, 4)}</td>
                  <td>{o.limit_price == null ? "—" : price(o.limit_price)}</td>
                  <td>{o.stop_price == null ? "—" : price(o.stop_price)}</td>
                  <td>{o.avg_fill_price == null ? "—" : price(o.avg_fill_price)}</td>
                  <td>{money(o.fees_usd)}</td>
                  <td className="left"><span className="pill neutral">{titleCase(o.status)}</span></td>
                  <td className="left muted">{dateTime(o.placed_at)}</td>
                </tr>
              ))}
            </tbody>
          </table></div>
        )}
      </Card>
    </div>
  );
}
