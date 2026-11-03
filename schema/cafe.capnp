@0xecc8c9717db0dfce;
# Versioned browser/server envelope. JSON payloads remain independently evolvable.
struct Envelope {
  version @0 :UInt16;
  method @1 :Text;
  token @2 :Text;
  payload @3 :Data;
}
