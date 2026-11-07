import '../../target/regressions/bindings.dart';
import '../../target/regressions/arrays.dart';
import '../../target/regressions/alpha.dart';
import 'dart:typed_data';

void main() {
  final view = ProjectionsView.create(Projections(vectors: [Vector(x: 1, y: 2, z: 3)]));
  final lanes = view.vectors;
  if (lanes.length != 4 || lanes[2] != 3 || lanes[3] != 0) throw StateError('padded projection');
  lanes[1] = 9;
  if (view.vectors[1] != 9) throw StateError('borrowed mutation');
  final arrays = FixedArraysView.create(FixedArrays(
    three: [1, 2, 3], four: [4, 5, 6, 7], matrix: [[8, 9, 10], [11, 12, 13]],
    present: [14, 15, 16], absent: null, readOnly: [17, 18, 19],
  ));
  final Float32List three = arrays.three;
  if (three.length != 3 || arrays.four.length != 4 || arrays.matrix.length != 6 || arrays.matrix[5] != 13) {
    throw StateError('fixed extents and nested flattening');
  }
  if (arrays.present![2] != 16 || arrays.absent != null) throw StateError('optional arrays');
  three[1] = 20;
  if (arrays.three[1] != 20) throw StateError('fixed view mutation');
  final cached = AlphaView.create(Alpha(values: [1, 2]));
  if (!identical(cached.values, cached.values)) throw StateError('cached collection view');
  print('Dart: numeric projections passed');
}
