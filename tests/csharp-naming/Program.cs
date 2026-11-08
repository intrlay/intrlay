using Intrlay;
using System;
using IntrlayGenerated.Schema;
using OmittedFactories = IntrlayGenerated.OmittedFactories;

var value = new NamingSampleInput(120, new Nullable<uint>(0));
var view = NamingSampleView.Static_Create(in value);
if (view.SampleRate != 120 || view.literalName != 0) throw new InvalidOperationException("named view read");
view.set_sample_rate_instance(144);
if (view.SampleRate != 144) throw new InvalidOperationException("named instance method");
if (view.Bytes.Length != NamingSampleView.Static_ComputeByteLength(in value)) throw new InvalidOperationException("named static method");
if (OmittedFactories.ChoiceView.byte_size_create != 4 || new OmittedFactories.ChoiceView(new byte[4]).Tag != 0)
    throw new InvalidOperationException("omitted factory name remains available for a constant");
Console.WriteLine("C#: naming rules preserve nullable API, explicit names, static/instance calls");
