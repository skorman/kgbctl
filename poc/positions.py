from ib_async import IB

HOST, PORT, CLIENT_ID = "127.0.0.1", 7496, 17

ib = IB()
ib.connect(HOST, PORT, clientId=CLIENT_ID, readonly=True)

positions = ib.positions()
print(f"{len(positions)} position(s)")
for p in positions:
    c = p.contract
    print(f"{p.account:10} {c.secType:5} {c.localSymbol or c.symbol:20} "
          f"qty={p.position:>10} avgCost={p.avgCost:>12.4f} {c.currency}")

ib.disconnect()
