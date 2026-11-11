import 'dart:typed_data';
import 'package:intrlay/intrlay.dart' as intrlay;
import '../../target/library-adapters/bindings.dart';
import '../../target/library-adapters/sample-data.dart';

void check(bool value) { if (!value) throw StateError('library adapter mismatch'); }

void main() {
  final view = SampleView(Uint8List.fromList(sampleBytes));
  check(view.boundsMinY == -2 && view.boundsMaxZ == 6);
  check(view.points.length == 8 && view.points[3] == 0 && view.points[4] == 10);
  check(view.matrix.yAxis.z == 6);
  check(view.mask.x && !view.mask.y && view.mask.z);
  check(view.labels[1] == 'two 🦀');
  final rebuilt = SampleView.create(SampleInput(
    bounds: const BoundsInput(min: Vec3AInput(x: -1, y: -2, z: -3), max: Vec3AInput(x: 4, y: 5, z: 6)),
    points: intrlay.SequenceInput.from(Float32List.fromList([7, 8, 9, 0, 10, 11, 12, 0])),
    matrix: const Mat3AInput(xAxis: Vec3AInput(x: 1, y: 2, z: 3), yAxis: Vec3AInput(x: 4, y: 5, z: 6), zAxis: Vec3AInput(x: 7, y: 8, z: 9)),
    mask: const BVec3AInput(x: true, y: false, z: true), labels: ['one', 'two 🦀']));
  check(rebuilt.storage.bytes.length == sampleBytes.length);
  for (var i = 0; i < sampleBytes.length; ++i) { check(rebuilt.storage.bytes[i] == sampleBytes[i]); }
  rebuilt.boundsMinY = -20;
  rebuilt.points[4] = 100;
  check(rebuilt.boundsMinY == -20 && rebuilt.points[4] == 100);
}
