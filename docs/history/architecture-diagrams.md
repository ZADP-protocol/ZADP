# Preserved architecture diagrams

> **Historical document.** These original diagrams are retained unchanged. Their mock/planned labels describe the earlier reference milestone, not current production. See [current architecture](../architecture.md).

```mermaid
flowchart TB
  subgraph discovery [ZADP Discovery - CURRENT]
    SellerPub[Seller publishes signed OFFER memo]
    IndexerScan[Indexer scans / validates / indexes]
    Api[Read-only multi-board API GET /offers]
    SellerPub --> IndexerScan --> Api
  end

  subgraph commerce [ZADP Commerce - MOCK DEMO]
    Quote[Buyer selects offer]
    Invoice[Seller issues invoice]
    Quote --> Invoice
  end

  subgraph settlement [Shielded Settlement - PLANNED / MOCK]
    Pay[Buyer payment controller]
    Adapter[ZcashAdapter]
    Observe[Seller observes payment]
    Pay --> Adapter --> Observe
  end

  discovery -.-> commerce -.-> settlement
```

```mermaid
flowchart LR
  BrowserGPS[Seller browser GPS - explicit click] --> Round[Local 3-decimal privacy rounding]
  Round --> Center[Public Offer Area - seller reviews or edits]
  Manual[Seller-selected approximate center] --> Center
  Center --> Offer[Signed Offer v2 geo]
  Offer --> Chain[Shielded Zcash memo]
  Chain --> Index[Read-only board UIVK indexer]
  Index --> API[GET /offers - public area data]
  subgraph client [Buyer browser or agent - private client boundary]
    GPS[Local location - memory only]
    Feed[Downloaded public offers]
    Match[Pure local Haversine intersection]
    GPS --> Match
    Feed --> Match
  end
  API --> Feed
```

```mermaid
flowchart LR
  Id[Seller Ed25519 identity]
  Bind[bind_offer board_id + issued_at]
  Memo[512-byte ZIP 302 memo]
  Zallet[Local Zallet PCZT RPC]
  LwdSend[lightwalletd SendTransaction]
  Chain[Zcash network]

  Id --> Bind --> Memo --> Zallet --> LwdSend --> Chain
```

```mermaid
flowchart LR
  Board[Signed board.json]
  Scan[RealZcashScanner BoardSource]
  Lwd[CompactTxStreamer / lightwalletd]
  Idx[BoardIndexer]
  Sql[SqliteOfferRepository]
  Api[board_router]

  Board --> Scan
  Lwd --> Scan --> Idx --> Sql --> Api
```

```mermaid
flowchart LR
  SellerCli[zadp-seller CLI]
  Demo[zadp-demo]
  Api[zadp-api]
  Indexer[zadp-indexer]
  Payment[zadp-payment]
  Protocol[zadp-protocol]
  Zcash[zadp-zcash]

  SellerCli --> Protocol
  SellerCli --> Zcash
  Demo --> Api
  Demo --> Payment
  Demo --> Protocol
  Indexer --> Protocol
  Indexer --> Zcash
  Api --> Indexer
  Api --> Protocol
  Payment --> Protocol
  Payment --> Zcash
```

```mermaid
flowchart TB
  subgraph may_hold [May hold]
    SellerSpend[Seller spending key - Zallet only]
    BuyerSpend[Buyer spending key - payment controller only]
    UIVK[Board UIVK - indexer / public descriptor]
    SellerPubkey[Seller Ed25519 identity - public]
  end

  subgraph must_not [Must NEVER hold]
    LLM[LLM / agent process]
  end

  LLM -->|request_payment invoice| BuyerSpend
  UIVK --> IndexerProc[Indexer process]
```
