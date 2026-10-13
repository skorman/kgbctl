import re

fname = "gaia.csv"


with open(fname, "r") as f:
    fluff = f.readlines()
wub = [x.strip() for x in fluff if x.strip() and x != "Ticker"]
print(wub)

with open(fname, "w") as w:
    w.write(",".join(wub))
