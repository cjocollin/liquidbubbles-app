import 'package:flutter_test/flutter_test.dart';
import 'package:bluebubbles/helpers/types/constants.dart';

// Note: Full integration tests require a Flutter environment with platform channels.
// These are basic unit tests for the data structures and constants.

void main() {
  group('GMPairingState', () {
    test('has all expected values', () {
      expect(GMPairingState.values.length, 5);
      expect(GMPairingState.values.contains(GMPairingState.notPaired), true);
      expect(GMPairingState.values.contains(GMPairingState.waitingForPairing), true);
      expect(GMPairingState.values.contains(GMPairingState.paired), true);
      expect(GMPairingState.values.contains(GMPairingState.expired), true);
      expect(GMPairingState.values.contains(GMPairingState.error), true);
    });

    test('notPaired is the default state', () {
      // notPaired should be first in the enum (index 0)
      expect(GMPairingState.values.first, GMPairingState.notPaired);
    });

    test('can convert to string', () {
      expect(GMPairingState.notPaired.toString(), 'GMPairingState.notPaired');
      expect(GMPairingState.paired.toString(), 'GMPairingState.paired');
    });

    test('can parse from index', () {
      expect(GMPairingState.values[0], GMPairingState.notPaired);
      expect(GMPairingState.values[1], GMPairingState.waitingForPairing);
      expect(GMPairingState.values[2], GMPairingState.paired);
      expect(GMPairingState.values[3], GMPairingState.expired);
      expect(GMPairingState.values[4], GMPairingState.error);
    });
  });

  group('GMPairingState serialization', () {
    test('index can be used for serialization', () {
      // Test that we can serialize via index
      final state = GMPairingState.paired;
      final serialized = state.index;
      final deserialized = GMPairingState.values[serialized];
      expect(deserialized, state);
    });

    test('all states can be round-tripped', () {
      for (final state in GMPairingState.values) {
        final serialized = state.index;
        final deserialized = GMPairingState.values[serialized];
        expect(deserialized, state);
      }
    });
  });
}
