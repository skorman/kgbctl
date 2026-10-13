import asyncio
from collections import defaultdict
from pathlib import Path

import kraken
from ib_async import IB
from rich.text import Text
from textual.app import App, ComposeResult
from textual.containers import Horizontal, Vertical
from textual.widgets import (ContentSwitcher, DataTable, Footer, Header, Static, TabbedContent,
                             TabPane)

HOST, PORT, CLIENT_ID = "127.0.0.1", 7496, 18
ETFS = ["zeus", "ares", "prometheus", "gaia"]
SOURCES = ["Interactive Brokers", "Kraken"]
SPINNER = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"
TABS = [*ETFS, "other", "crypto"]
HERE = Path(__file__).parent
COLUMNS = ["Ticker", "Value ($)", "Shares", "Cost", "Price", "Weight", "% P/L"]


def load_tickers(etf: str) -> set[str]:
    text = (HERE / f"{etf}.csv").read_text()
    return {t.strip() for t in text.split(",") if t.strip()}


async def fetch_holdings() -> dict[str, tuple[float, float, float, float]]:
    """ticker -> (value, shares, avg cost, price), summed across accounts."""
    ib = IB()
    await ib.connectAsync(HOST, PORT, clientId=CLIENT_ID, readonly=True, timeout=15)
    await asyncio.sleep(1)  # let the portfolio updates arrive
    agg = defaultdict(lambda: [0.0, 0.0, 0.0])
    for item in ib.portfolio():
        sym = item.contract.symbol
        agg[sym][0] += item.marketValue
        agg[sym][1] += item.position
        agg[sym][2] += item.averageCost * item.position  # total cost basis
    prices = {i.contract.symbol: i.marketPrice for i in ib.portfolio()}
    ib.disconnect()
    return {s: (v, q, c / q if q else 0.0, prices[s]) for s, (v, q, c) in agg.items()}


def bucket(holdings):
    """etf name -> list of tickers. A ticker listed in several ETFs shows in each."""
    members = {etf: load_tickers(etf) for etf in ETFS}
    buckets = {etf: [] for etf in ETFS} | {"other": []}
    for sym in holdings:
        hit = [e for e in ETFS if sym in members[e]]
        for e in hit or ["other"]:
            buckets[e].append(sym)
    return buckets


def price_str(x: float) -> str:
    return f"{x:,.2f}" if x >= 1 else f"{x:,.4f}"


def money(x: float) -> str:
    return f"{'-' if x < 0 else ''}${abs(x):,.2f}"


class Card(Static):
    DEFAULT_CSS = """
    Card { border: round $primary; width: 1fr; height: 5; padding: 0 1; content-align: center middle; }
    """


class KgbPoc(App):
    CSS = """
    .cards { height: 5; }
    #breakdown { padding: 1 2; }
    """
    BINDINGS = [
        ("q", "quit", "Quit"),
        ("r", "refresh", "Refresh"),
        ("o", "toggle_view", "Overview / Detail"),
        ("left", "tab(-1)", "Prev ETF"),
        ("right", "tab(1)", "Next ETF"),
    ]

    def compose(self) -> ComposeResult:
        yield Header()
        yield Static("Connecting to TWS...", id="status")
        with ContentSwitcher(initial="overview", id="views"):
            with Vertical(id="overview"):
                with Horizontal(classes="cards"):
                    yield Card(id="card-value-all")
                    yield Card(id="card-pl-all")
                    yield Card(id="card-plpct-all")
                yield Static(id="breakdown")
            yield from self.compose_tabs()
        yield Footer()

    def compose_tabs(self) -> ComposeResult:
        with TabbedContent(id="etf-tabs"):
            for name in TABS:
                with TabPane(name.capitalize(), id=name):
                    with Horizontal(classes="cards"):
                        yield Card(id=f"card-value-{name}")
                        yield Card(id=f"card-pl-{name}")
                        yield Card(id=f"card-plpct-{name}")
                    table = DataTable(id=f"t-{name}", cursor_type="row", zebra_stripes=True)
                    table.add_columns(*COLUMNS)
                    yield table

    sources: dict[str, dict] = {}
    loading: set[str] = set()
    frame = 0

    def on_mount(self) -> None:
        self.title = "kgbpoc"
        self.sources = {}
        self.loading = set()
        self.set_interval(0.1, self.tick)
        self.action_refresh()

    @property
    def etf(self) -> str:
        return self.query_one("#etf-tabs", TabbedContent).active

    @property
    def in_overview(self) -> bool:
        return self.query_one("#views", ContentSwitcher).current == "overview"

    def action_toggle_view(self) -> None:
        switcher = self.query_one("#views", ContentSwitcher)
        switcher.current = "etf-tabs" if self.in_overview else "overview"
        self.focus_active_table()

    def action_tab(self, step: int) -> None:
        if self.in_overview:
            return
        tabs = self.query_one("#etf-tabs", TabbedContent)
        tabs.active = TABS[(TABS.index(tabs.active) + step) % len(TABS)]

    def on_tabbed_content_tab_activated(self, event: TabbedContent.TabActivated) -> None:
        self.focus_active_table()

    def focus_active_table(self) -> None:
        if self.etf and not self.in_overview:
            self.query_one(f"#t-{self.etf}", DataTable).focus()

    def tick(self) -> None:
        if self.loading:
            self.frame += 1
            self.populate_overview()

    def action_refresh(self) -> None:
        self.loading = set(SOURCES)
        self.sources = {}
        self.populate_overview()
        self.query_one("#status", Static).update("Loading positions (crypto history can take ~30s)...")
        self.run_worker(self.load(), exclusive=True)

    async def load(self) -> None:
        # Independent sources: a failure in one must not blank the other's tabs.
        results = await asyncio.gather(
            self.load_ibkr(), self.load_crypto(), return_exceptions=True
        )
        errors = [f"{src}: {r!r}" for src, r in zip(("IBKR", "Kraken"), results)
                  if isinstance(r, Exception)]
        status = "r=refresh, q=quit"
        self.query_one("#status", Static).update(
            f"{status}  |  Error: {'; '.join(errors)}" if errors else status
        )
        self.focus_active_table()

    async def load_ibkr(self) -> None:
        try:
            holdings = await fetch_holdings()
        finally:
            self.loading.discard("Interactive Brokers")
        for name, syms in bucket(holdings).items():
            self.populate(name, syms, holdings)
        self.sources["Interactive Brokers"] = holdings
        self.populate_overview()

    async def load_crypto(self) -> None:
        try:
            holdings = await asyncio.to_thread(kraken.fetch_holdings)  # blocking urllib calls
        finally:
            self.loading.discard("Kraken")
        self.populate("crypto", list(holdings), holdings)
        self.sources["Kraken"] = holdings
        self.populate_overview()

    def populate(self, name, syms, holdings) -> None:
        table = self.query_one(f"#t-{name}", DataTable)
        table.clear()
        rows = sorted(syms, key=lambda s: -holdings[s][0])
        total = sum(holdings[s][0] for s in rows) or 1.0
        for s in rows:
            value, shares, cost, price = holdings[s]
            pl = (price / cost - 1) if cost else 0.0
            table.add_row(
                s, f"{value:,.2f}", f"{shares:,.4f}", price_str(cost), price_str(price),
                f"{value / total:.1%}",
                Text(f"{pl:+.2%}", style="green" if pl >= 0 else "red"),
            )
        self.populate_cards(name, rows, holdings)

    @staticmethod
    def totals(holdings, syms=None) -> tuple[float, float]:
        """(value, cost basis) over syms (default: all) of one holdings dict."""
        syms = holdings if syms is None else syms
        return (sum(holdings[s][0] for s in syms),
                sum(holdings[s][1] * holdings[s][2] for s in syms))

    def populate_overview(self) -> None:
        value = basis = 0.0
        lines = Text()
        spin = SPINNER[self.frame % len(SPINNER)]
        for src in SOURCES:
            if src in self.loading:
                lines.append(f"{src:<22} {spin} loading...\n", style="dim")
                continue
            if src not in self.sources:
                lines.append(f"{src:<22} unavailable\n", style="red")
                continue
            holdings = self.sources[src]
            v, b = self.totals(holdings)
            value, basis = value + v, basis + b
            pl = v - b
            lines.append(f"{src:<22} {money(v):>14}   ")
            lines.append(f"{'+' if pl >= 0 else ''}{money(pl)} ({pl / b if b else 0:+.2%})\n",
                         style="green" if pl >= 0 else "red")
        self.set_cards("all", value, basis, spin if self.loading else "")
        self.query_one("#breakdown", Static).update(lines)

    def populate_cards(self, name, syms, holdings) -> None:
        self.set_cards(name, *self.totals(holdings, syms))

    def set_cards(self, name, value, basis, spin="") -> None:
        pl = value - basis
        pct = pl / basis if basis else 0.0
        style = "green" if pl >= 0 else "red"
        for key, label, text, st in [
            ("value", "Total AUM" if name == "all" else "Total Value", money(value), "bold"),
            ("pl", "Total P/L $", f"{'+' if pl >= 0 else ''}{money(pl)}", f"bold {style}"),
            ("plpct", "Total P/L %", f"{pct:+.2%}", f"bold {style}"),
        ]:
            card = Text(f"{label} {spin}\n".replace(" \n", "\n"), style="dim")
            card.append(text, style=st)
            self.query_one(f"#card-{key}-{name}", Static).update(card)


if __name__ == "__main__":
    KgbPoc().run()
