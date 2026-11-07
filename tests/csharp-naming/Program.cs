using Intrlay;
using System;
using IntrlayGenerated.Schema;

var value = new NamingSample(120, new Nullable<uint>(0));
var view = NamingSampleView.Static_Create(in value);
if (view.SampleRate != 120 || view.literalName != 0) throw new InvalidOperationException("named view read");
view.set_sample_rate_instance(144);
if (view.SampleRate != 144) throw new InvalidOperationException("named instance method");
if (view.Bytes.Length != NamingSampleView.Static_ComputeByteLength(in value)) throw new InvalidOperationException("named static method");
Console.WriteLine("C#: naming rules preserve nullable API, explicit names, static/instance calls");
