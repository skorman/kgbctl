"""Proof of concept: read positions from Kraken using a read-only API key.

Needs KRAKEN_API_KEY and KRAKEN_SECRET_KEY in the environment. The key only
needs the "Query Funds" permission (plus "Query open orders & trades" for
margin positions).
"""
import base64
import hashlib
import hmac
import json
import os
import re
import time
from collections import defaultdict
import urllib.parse
import urllib.request

BASE = "https://api.kraken.com"
FIAT = {"USD", "EUR", "GBP", "CAD", "AUD", "CHF", "JPY"}


def public(path: str, **params) -> dict:
    url = f"{BASE}/0/public/{path}?{urllib.parse.urlencode(params)}"
    with urllib.request.urlopen(url, timeout=15) as r:
        return check(json.load(r))


def private(path: str, **params) -> dict:
    urlpath = f"/0/private/{path}"
    data = {"nonce": str(time.time_ns() // 1000), **params}
    postdata = urllib.parse.urlencode(data)
    digest = hashlib.sha256((data["nonce"] + postdata).encode()).digest()
    mac = hmac.new(base64.b64decode(os.environ["KRAKEN_SECRET_KEY"]),
                   urlpath.encode() + digest, hashlib.sha512)
    req = urllib.request.Request(
        BASE + urlpath, data=postdata.encode(),
        headers={"API-Key": os.environ["KRAKEN_API_KEY"],
                 "API-Sign": base64.b64encode(mac.digest()).decode()},
    )
    with urllib.request.urlopen(req, timeout=15) as r:
        return check(json.load(r))


def check(resp: dict) -> dict:
    if resp.get("error"):
        raise RuntimeError(f"Kraken error: {resp['error']}")
    return resp["result"]


def base_asset(asset: str) -> str:
    """XXBT -> XBT, ZUSD -> USD, ETH.S / SOL03.S / SUI.B / XETH.F -> ETH / SOL / SUI / ETH."""
    asset = asset.split(".")[0]
    if len(asset) == 4 and asset[0] in "XZ":
        asset = asset[1:]
    return re.sub(r"\d+$", "", asset)


def display_symbol(sym: str) -> str:
    return {"XBT": "BTC"}.get(sym, sym)


def usd_price(sym: str) -> float | None:
    if sym == "USD":
        return 1.0
    try:
        res = public("Ticker", pair=f"{sym}USD")
        return float(next(iter(res.values()))["c"][0])
    except Exception:
        return None


def fetch_all(path: str, key: str) -> list[dict]:
    """Page through a private history endpoint (50 records per call)."""
    out, ofs = [], 0
    while True:
        r = private(path, ofs=ofs)
        out += list(r[key].values())
        ofs = len(out)
        if not r[key] or ofs >= r["count"]:
            return out
        time.sleep(3)  # stay under the rate limit; ledger calls are costly


class Lot:
    """Running position for one base asset, using the average-cost method."""

    def __init__(self) -> None:
        self.qty = 0.0         # net quantity held, from the ledger
        self.cost = 0.0        # USD cost basis of that quantity
        self.bought_qty = 0.0  # lifetime quantity bought for USD
        self.bought_usd = 0.0  # lifetime USD spent on those buys (incl. fees)
        self.free_qty = 0.0    # staking rewards / bonuses (cost basis $0)
        self.in_qty = 0.0      # deposited from outside (cost basis unknown, taken as $0)
        self.realized = 0.0

    def buy(self, qty: float, usd: float) -> None:
        self.qty += qty
        self.cost += usd
        self.bought_qty += qty
        self.bought_usd += usd

    def sell(self, qty: float, usd: float) -> None:
        avg = self.cost / self.qty if self.qty else 0.0
        self.qty -= qty
        self.cost -= avg * qty
        self.realized += usd - avg * qty

    def free(self, qty: float, kind: str) -> None:
        self.qty += qty
        if kind == "reward":
            self.free_qty += qty
        else:
            self.in_qty += qty

    @property
    def avg_cost(self) -> float:
        return self.cost / self.qty if self.qty else 0.0

    @property
    def avg_buy_price(self) -> float:
        return self.bought_usd / self.bought_qty if self.bought_qty else 0.0


def build_lots(ledger: list[dict]) -> dict[str, Lot]:
    """Replay the ledger oldest-first. Legs sharing a refid form one trade/convert."""
    groups: dict[str, list[dict]] = defaultdict(list)
    for e in sorted(ledger, key=lambda e: e["time"]):
        groups[e["refid"]].append(e)
    lots: dict[str, Lot] = defaultdict(Lot)

    for legs in sorted(groups.values(), key=lambda g: g[0]["time"]):
        kinds = {e["type"] for e in legs}
        net = lambda e: float(e["amount"]) - float(e["fee"])
        usd = [e for e in legs if base_asset(e["asset"]) == "USD"]
        crypto = [e for e in legs if base_asset(e["asset"]) != "USD"]

        if kinds & {"trade", "receive", "spend"} and usd:
            if len({base_asset(e["asset"]) for e in crypto}) != 1:
                continue  # crypto<->crypto or wrapper-only moves: nothing to price
            sym = base_asset(crypto[0]["asset"])
            qty = sum(net(e) for e in crypto)
            usd_flow = sum(net(e) for e in usd)  # fees folded in: negative on buys
            if qty > 0:
                lots[sym].buy(qty, -usd_flow)
            elif qty < 0:
                lots[sym].sell(-qty, usd_flow)
        elif kinds == {"transfer"} or "earn" in kinds:
            continue  # spot <-> staked wrapper moves net to zero for the base asset
        else:
            for e in crypto:
                sym = base_asset(e["asset"])
                if e["type"] in ("staking", "reward"):
                    lots[sym].free(net(e), "reward")
                elif e["type"] in ("deposit", "receive"):
                    lots[sym].free(net(e), "deposit")
                elif e["type"] == "withdrawal":
                    lots[sym].sell(-net(e), 0.0) if net(e) < 0 else None
    return lots


def fetch_holdings() -> dict[str, tuple[float, float, float, float]]:
    """ticker -> (value, qty, avg cost, price) for crypto assets, same shape as the IBKR view."""
    balances: dict[str, float] = defaultdict(float)
    for asset, v in private("Balance").items():
        balances[base_asset(asset)] += float(v)
    lots = build_lots(fetch_all("Ledgers", "ledger"))
    out = {}
    for sym, qty in balances.items():
        if sym == "USD" or qty == 0:
            continue
        price = usd_price(sym) or 0.0
        out[display_symbol(sym)] = (qty * price, qty, lots[sym].avg_cost, price)
    return out


def main() -> None:
    balances: dict[str, float] = defaultdict(float)
    for asset, v in private("Balance").items():
        balances[base_asset(asset)] += float(v)
    lots = build_lots(fetch_all("Ledgers", "ledger"))

    print(f"{'Asset':6} {'Qty':>14} {'Ledger qty':>14} {'Avg cost':>12} {'Avg buy':>12} "
          f"{'Price':>12} {'Value':>11} {'Cost':>11} {'P/L $':>10} {'P/L %':>8}")
    tot_val = tot_cost = 0.0
    for sym in sorted(balances, key=lambda s: -balances[s]):
        qty = balances[sym]
        if sym == "USD" or qty == 0:
            continue
        lot, price = lots[sym], usd_price(sym)
        value = qty * price if price else 0.0
        cost = lot.avg_cost * qty
        tot_val += value
        tot_cost += cost
        pl = value - cost
        print(f"{display_symbol(sym):6} {qty:14.6f} {lot.qty:14.6f} {lot.avg_cost:12,.4f} "
              f"{lot.avg_buy_price:12,.4f} {price or 0:12,.4f} {value:11,.2f} {cost:11,.2f} "
              f"{pl:+10,.2f} {pl / cost if cost else 0:+8.1%}")
    print(f"{'TOTAL':6} {'':14} {'':14} {'':12} {'':12} {'':12} {tot_val:11,.2f} {tot_cost:11,.2f} "
          f"{tot_val - tot_cost:+10,.2f} {(tot_val / tot_cost - 1) if tot_cost else 0:+8.1%}")
    print("\nAvg cost = blended basis (rewards at $0, deposits at $0); Avg buy = USD paid per coin bought.")
    for sym, lot in lots.items():
        if sym != "USD" and (lot.free_qty or lot.in_qty):
            print(f"  {display_symbol(sym)}: bought {lot.bought_qty:.6f}, staking rewards "
                  f"{lot.free_qty:.6f}, deposited-in {lot.in_qty:.6f}, realized P/L ${lot.realized:+,.2f}")

    margin = private("OpenPositions")
    print(f"\n{len(margin)} open margin position(s)")


if __name__ == "__main__":
    main()
