# Reading IBKR Positions from TWS: Feasibility

Oct 12, 2026 · @Sam

## Short answer

Reading your positions automatically from Trader Workstation (TWS) is very feasible. TWS has a built-in local socket API that streams positions in real time, and a read-only TUI that shows positions is a well-trodden path. Use the socket API for live positions and P&L, and IBKR's Flex Web Service for trade history.

## Recommended approach: the TWS socket API

TWS and IB Gateway both expose a local socket API. Your app connects to it on localhost and receives portfolio data directly.

| Application | Paper port | Live port |
| --- | --- | --- |
| TWS | 7497 | 7496 |
| IB Gateway | 4002 | 4001 |

One source I found had the TWS live and paper ports swapped. Use IBKR's defaults above and confirm the port in your TWS settings.

The calls that matter for a position tracker:

- **reqPositions**: returns all positions across all associated accounts, then a positionEnd callback, then pushes updates whenever a position changes. This is the simplest fit.
- **reqAccountUpdates**: positions plus account values in one subscription, matching the TWS Account Window. It refreshes every three minutes unless a position changes, and only one managed account can be subscribed at a time.
- **reqPnLSingle / reqPnL**: real-time daily and unrealized P&L per position or account, matching the TWS Portfolio Window. Account-level P&L needs the TWS setting "Prepare portfolio PnL data when downloading positions" enabled.
- **reqAccountSummary**: balances and margin figures, without positions.

**Setup on your Mac:** in TWS, open Global Configuration, then API, then Settings. Check "Enable ActiveX and Socket Clients" and set the socket port. Restrict connections to localhost (127.0.0.1). Keep "Read-Only API" checked: it blocks order placement from your code, and you only need to read positions.

## Library choices

- **Python: ib\_async** is my pick. It is the community-maintained successor to ib\_insync and implements the full IBKR API protocol internally. The original author passed away in 2024, which is why it was forked. The last push I found was May 1, 2026, with about 1.5k stars. It pairs naturally with a Python TUI framework such as Textual.
- **Node/TypeScript: @stoqey/ib** is a port of the official Java client, and its examples call reqPositions directly.
- **Official ibapi** from IBKR also works, but it is callback-heavy and more verbose.

## Alternatives and why they are second choice

| Option | What it is | Trade-off |
| --- | --- | --- |
| Client Portal Web API | REST API behind a local gateway | Daily browser login with 2FA each time. More friction than the socket API with no benefit for local use. The OAuth-based Unified Web API avoids the gateway but is subject to eligibility. |
| Flex Web Service | Report-based XML fetched with a token and query ID, including Open Positions | Best for historical trades and activity. Not live, so it complements the socket API rather than replacing it. |
| IB Gateway instead of TWS | Lighter, API-only application | Optional, since you already run TWS. |

Two limits matter for a trade tracker. TWS does not provide historical portfolio information to the API by design, and the TWS API cannot retrieve an account's historical trades. The bankroll project uses the Flex Web Service for that. A sensible design is the socket API for live positions and P&L, and Flex Queries for trade history and closed-trade stats.

## Caveats to plan for

1. **TWS must be running and logged in.** Your app is a client of it, so if TWS closes or its session drops, the TUI loses data. Handle disconnects and reconnects.
2. **Daily re-authentication.** IB Gateway requires daily restarts or re-authentication, with 2FA. Tools like IBC can automate restarts, but with TWS on your Mac you will likely log in each day.
3. **Client IDs must be unique** per connection to the same TWS instance. Pick a fixed one for your app.
4. **P&L figures can disagree** between the Account Window and Portfolio Window sources. Pick one source and label it in your UI.
5. **Security:** keep the API bound to localhost, keep it read-only, and keep Flex tokens out of version control.

## Suggested next step

Build a minimal prototype: connect with ib\_async to 127.0.0.1:7496, call reqPositions, and print the results. If it works, wire it into the TUI's data layer behind an interface, so a Flex Query importer for history can be added later. I have not tested any of this against your TWS setup, so check the official docs for your TWS version.

## Sources

- [TWS API: Positions](https://www.interactivebrokers.com/docs/tws-api/doc/account-portfolio-data/positions/introduction)
- [TWS API: Account Updates](https://www.interactivebrokers.com/docs/tws-api/doc/account-portfolio-data/account-updates/introduction)
- [TWS API: Profit & Loss](https://interactivebrokers.github.io/tws-api/pnl.html)
- [IBKR Campus: Account and Portfolio Data lesson](https://www.interactivebrokers.com/campus/trading-lessons/python-account-portfolio/)
- [IBKR Campus: Client Portal Gateway authentication](https://ibkrcampus.com/?p=203297)
- [IBKR Web API documentation](https://www.interactivebrokers.com/docs/web-api/web-api-v-1-0-documentation/o-auth-1-0-a/live-session-token/endpoint)
- [ib\_async on GitHub](https://github.com/ClimberMel/ib_async) and [PyPI](https://pypi.org/project/ib_async)
- [ib\_async repo stats (gittrend)](https://gittrend.io/repo/ib-api-reloaded/ib_async)
- [@stoqey/ib](https://awesome.ecosyste.ms/projects/github.com%2Fstoqey%2Fib)
- [etfray IBKR setup guide](https://etfray.readthedocs.io/en/latest/user-guide/ibkr-setup/)
- [NautilusTrader IB docs (ports)](https://nautilustrader.io/docs/latest/integrations/interactive_brokers/)
- [bankroll (Flex Web Service usage)](https://pypi.org/project/bankroll)
- [ib-flex (Flex Query parsing)](https://docs.rs/crate/ib-flex/0.2.1/source/README.md)
- [BrokerListings: Does Interactive Brokers have an API?](https://brokerlistings.com/reviews/interactive-brokers/does-interactive-brokers-have-an-api)
