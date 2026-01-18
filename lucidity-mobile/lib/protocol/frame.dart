import 'dart:typed_data';

import 'constants.dart';

class Frame {
  final int type;
  final Uint8List payload;

  const Frame({required this.type, required this.payload});
}

Uint8List encodeFrame({required int type, required List<int> payload}) {
  final len = payload.length + 1;
  if (len <= 0) {
    throw ArgumentError('invalid frame length: $len');
  }
  if (len > maxFrameLen) {
    throw ArgumentError('frame too large: $len > $maxFrameLen');
  }

  final out = BytesBuilder(copy: false);
  final header = ByteData(4);
  header.setUint32(0, len, Endian.little);
  out.add(header.buffer.asUint8List());
  out.addByte(type & 0xff);
  out.add(payload);
  return out.takeBytes();
}

class FrameDecoder {
  Uint8List _data = Uint8List(0);
  int _offset = 0;

  int get bufferedLen => (_data.length - _offset);

  void push(Uint8List data) {
    if (data.isEmpty) return;

    // Compact existing buffer if we've consumed some.
    if (_offset > 0) {
      _data = Uint8List.fromList(_data.sublist(_offset));
      _offset = 0;
    }

    if (_data.isEmpty) {
      _data = Uint8List.fromList(data);
      return;
    }

    final next = Uint8List(_data.length + data.length);
    next.setRange(0, _data.length, _data);
    next.setRange(_data.length, next.length, data);
    _data = next;
  }

  Frame? nextFrame() {
    if (bufferedLen < 4) return null;

    final bd = ByteData.sublistView(_data, _offset, _offset + 4);
    final len = bd.getUint32(0, Endian.little);
    if (len > maxFrameLen) {
      throw StateError('declared length $len exceeds max $maxFrameLen');
    }
    if (len == 0) {
      throw StateError('declared length $len is invalid');
    }

    final total = 4 + len;
    if (bufferedLen < total) return null;

    final type = _data[_offset + 4];
    final payloadStart = _offset + 5;
    final payloadEnd = _offset + total;
    final payload = Uint8List.fromList(_data.sublist(payloadStart, payloadEnd));
    _offset += total;
    return Frame(type: type, payload: payload);
  }
}
