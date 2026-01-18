import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';

import 'package:lucidity_mobile/protocol/constants.dart';
import 'package:lucidity_mobile/protocol/frame.dart';

void main() {
  test('encodeFrame + FrameDecoder roundtrip (single chunk)', () {
    final payload = Uint8List.fromList([1, 2, 3, 4, 5]);
    final bytes = encodeFrame(type: typeJson, payload: payload);

    final dec = FrameDecoder();
    dec.push(bytes);

    final frame = dec.nextFrame();
    expect(frame, isNotNull);
    expect(frame!.type, typeJson);
    expect(frame.payload, payload);

    expect(dec.nextFrame(), isNull);
  });

  test('FrameDecoder decodes across chunks', () {
    final payload = Uint8List.fromList(List.generate(100, (i) => i & 0xff));
    final bytes = encodeFrame(type: typePaneOutput, payload: payload);

    final dec = FrameDecoder();
    dec.push(bytes.sublist(0, 3)); // not enough for length
    expect(dec.nextFrame(), isNull);

    dec.push(bytes.sublist(3, 10)); // enough for header, not full frame
    expect(dec.nextFrame(), isNull);

    dec.push(bytes.sublist(10)); // rest
    final frame = dec.nextFrame();
    expect(frame, isNotNull);
    expect(frame!.type, typePaneOutput);
    expect(frame.payload, payload);
  });

  test('FrameDecoder rejects invalid lengths', () {
    final bad = Uint8List.fromList([
      0,
      0,
      0,
      0, // len = 0 (invalid)
      1, // typ
    ]);
    final dec = FrameDecoder();
    dec.push(bad);
    expect(() => dec.nextFrame(), throwsStateError);
  });
}

