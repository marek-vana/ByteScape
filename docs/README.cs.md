# ByteScape — rychlý návod

Experimentální mapa disku pro macOS. První vydání `0.1.0-alpha.1` obsahuje pouze
zdroje; podepsaná a notarizovaná aplikace zatím není k dispozici.

Ověřeno na M1 / 8 GiB, macOS 27.0.1 a Rustu 1.99.1. Nainstalujte Rust a Xcode
Command Line Tools. V terminálu:

```sh
git clone https://github.com/marek-vana/ByteScape.git
cd ByteScape
cargo run --locked --release
```

Po spuštění vidíte označená ukázková data. ⌘O vybere skutečnou složku, ⌘. zruší
sken a ⌘R jej zopakuje. Mapa, kruhy, seznam a detail zobrazují metadata;
obsah souborů se nečte. Symbolické odkazy se nenásledují, pevné odkazy se
započítají jednou. Výsledky se mezi spuštěními neukládají. Mazání není součástí
aplikace. Přidělené místo neznamená prostor, který lze uvolnit.

Přepnutí CZ/EN, zvětšení UI a omezení pohybu je v aplikaci.
Před použitím si přečtěte [omezení](limitations.md), zejména neověřené cloudové
a externí integrace. [Anglické README](../README.md) obsahuje vývojové příkazy.
