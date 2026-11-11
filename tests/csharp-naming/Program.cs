using Intrlay;
using System;
using IntrlayGenerated.Schema;
using OmittedFactories = IntrlayGenerated.OmittedFactories;
using EnumNames = IntrlayGenerated.EnumNames;
using PayloadNames = IntrlayGenerated.PayloadNames;

var value = new NamingSampleInput(120, new Nullable<uint>(0));
var view = NamingSampleView.Static_Create(in value);
if (view.SampleRate != 120 || view.literalName != 0) throw new InvalidOperationException("named view read");
view.set_sample_rate_instance(144);
if (view.SampleRate != 144) throw new InvalidOperationException("named instance method");
if (view.Bytes.Length != NamingSampleView.Static_ComputeByteLength(in value)) throw new InvalidOperationException("named static method");
if (OmittedFactories.ChoiceView.byte_size_create != 4 || new OmittedFactories.ChoiceView(new byte[4]).Tag != 0)
    throw new InvalidOperationException("omitted factory name remains available for a constant");
var namedEnums = EnumNames.PacketView.Create(new EnumNames.PacketInput(
    EnumNames.buffer.Ready, EnumNames.offset.Ready, EnumNames.cursor.Ready, EnumNames.value.Ready));
if (namedEnums.Buffer != EnumNames.buffer.Ready || namedEnums.Offset != EnumNames.offset.Ready
    || namedEnums.Cursor != EnumNames.cursor.Ready || namedEnums.Value != EnumNames.value.Ready)
    throw new InvalidOperationException("enum constants must resolve independently of codec parameters");
var payload = PayloadNames.PayloadEventView.Create(PayloadNames.PayloadEventInput.FromVariantTaskDone(7, 11));
if (payload.read_task_done_request_id != 7 || payload.read_task_done_saved_id != 11)
    throw new InvalidOperationException("input property prefix must not enter enum getter names");
payload.SetReadTaskDoneRequestId(19);
if (payload.AsSpan().read_task_done_request_id != 19)
    throw new InvalidOperationException("named payload setter");
payload.Replace(PayloadNames.PayloadEventInput.FromDelivered(23));
payload.SetReadDeliveredTicket(29);
if (payload.read_delivered_ticket != 29 || payload.read_task_done_request_id is not null)
    throw new InvalidOperationException("explicit variant and field names");
Console.WriteLine("C#: naming rules preserve nullable API, explicit names, static/instance calls");
