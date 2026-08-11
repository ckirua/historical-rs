#!/usr/bin/env python3
"""Download verified Bybit Spot/Contract orderbook/trade archives with resumable files."""
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import date, timedelta
from pathlib import Path
import json
import os
import time
import urllib.parse
import urllib.request


API = "https://api2.bybit.com/quote/public/support/download/list-files"
ROOT = Path(os.environ.get("BYBIT_DATA_DIR", Path.home() / ".data/bybit-historical"))
MARKET = os.environ.get("BYBIT_MARKET", "spot").strip().lower()
DATA_TYPE = os.environ.get("BYBIT_DATA_TYPE", "orderbook").strip().lower()
INTERVAL = os.environ.get("BYBIT_INTERVAL", "daily").strip().lower()
SYMBOLS = [
    symbol.strip().upper()
    for symbol in os.environ.get(
        "BYBIT_SYMBOLS", "BTCUSDT,ETHUSDT,SOLUSDT,XRPUSDT,DOGEUSDT"
    ).split(",")
    if symbol.strip()
]
FROM = date.fromisoformat(
    os.environ.get("BYBIT_FROM", (date.today() - timedelta(days=92)).isoformat())
)
TO = date.fromisoformat(
    os.environ.get("BYBIT_TO", (date.today() - timedelta(days=1)).isoformat())
)
WORKERS = int(os.environ.get("BYBIT_DOWNLOAD_WORKERS", "4"))
DATASET_ROOT = ROOT / DATA_TYPE / MARKET
STATUS = DATASET_ROOT / "download-status.jsonl"


def validate_dataset():
    if MARKET not in {"spot", "contract"} or DATA_TYPE not in {"orderbook", "trade"}:
        raise SystemExit(
            "unsupported Bybit dataset: "
            f"market={MARKET!r} data_type={DATA_TYPE!r}; "
            "verified combinations are market='spot'|'contract' with "
            "data_type='orderbook' or data_type='trade' "
            "(use portal bizType contract, not linear/inverse)"
        )
    if INTERVAL != "daily":
        raise SystemExit(
            f"unsupported Bybit interval {INTERVAL!r}; "
            "the verified portal contract supports interval='daily'"
        )


def request_json(url, retries=4):
    for attempt in range(retries):
        try:
            req = urllib.request.Request(
                url, headers={"User-Agent": "historical-rs/bybit-portal"}
            )
            with urllib.request.urlopen(req, timeout=30) as response:
                return json.load(response)
        except Exception:
            if attempt + 1 == retries:
                raise
            time.sleep(2**attempt)


def list_archives(symbol, day):
    query = urllib.parse.urlencode(
        {
            "bizType": MARKET,
            "productId": DATA_TYPE,
            "symbols": symbol,
            "interval": INTERVAL,
            "startDay": day.isoformat(),
            "endDay": day.isoformat(),
        }
    )
    payload = request_json(f"{API}?{query}")
    items = payload.get("result", {}).get("list", [])
    return [
        {
            "market": MARKET,
            "data_type": DATA_TYPE,
            "interval": INTERVAL,
            "symbol": symbol,
            "date": day.isoformat(),
            "url": item["url"],
            "filename": item["filename"],
            "size": int(item["size"]),
        }
        for item in items
    ]


def unavailable(symbol, day):
    return {
        "market": MARKET,
        "data_type": DATA_TYPE,
        "interval": INTERVAL,
        "symbol": symbol,
        "date": day.isoformat(),
        "url": None,
        "filename": None,
        "size": None,
        "status": "unavailable",
    }


def task(symbol, day):
    archives = list_archives(symbol, day)
    if not archives:
        return unavailable(symbol, day)
    archive = archives[0]
    target_dir = DATASET_ROOT / symbol / day.isoformat()
    target_dir.mkdir(parents=True, exist_ok=True)
    target = target_dir / archive["filename"]
    expected_size = archive["size"]
    if target.exists() and target.stat().st_size == expected_size:
        return {**archive, "status": "exists", "bytes": expected_size}
    partial = target.with_suffix(target.suffix + ".part")
    for attempt in range(4):
        try:
            urllib.request.urlretrieve(archive["url"], partial)
            size = partial.stat().st_size
            if size != expected_size:
                raise RuntimeError(
                    f"size mismatch expected={expected_size} actual={size}"
                )
            partial.replace(target)
            return {
                **archive,
                "status": "downloaded",
                "bytes": size,
                "file": str(target),
            }
        except Exception as exc:
            if attempt + 1 == 4:
                return {**archive, "status": "failed", "error": str(exc)}
            time.sleep(2**attempt)
    raise AssertionError("unreachable")


def main():
    validate_dataset()
    if FROM > TO:
        raise SystemExit("BYBIT_FROM must not be after BYBIT_TO")
    ROOT.mkdir(parents=True, exist_ok=True)
    days = [FROM + timedelta(days=i) for i in range((TO - FROM).days + 1)]
    jobs = [(symbol, day) for symbol in SYMBOLS for day in days]
    print(
        "Bybit "
        f"{MARKET} {DATA_TYPE} (portal productId={DATA_TYPE}): "
        f"{len(jobs)} files, symbols={','.join(SYMBOLS)}, "
        f"range={FROM}..{TO}, workers={WORKERS}",
        flush=True,
    )
    DATASET_ROOT.mkdir(parents=True, exist_ok=True)
    with STATUS.open("a", encoding="utf-8") as status:
        with ThreadPoolExecutor(max_workers=WORKERS) as pool:
            futures = [pool.submit(task, symbol, day) for symbol, day in jobs]
            for future in as_completed(futures):
                result = future.result()
                status.write(json.dumps(result, sort_keys=True) + "\n")
                status.flush()
                print(json.dumps(result, sort_keys=True), flush=True)


if __name__ == "__main__":
    main()
