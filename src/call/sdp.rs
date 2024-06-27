/**
v=0
o=Z 0 2120575 IN IP4 123.16.85.0
s=Z
c=IN IP4 14.225.211.34
t=0 0
m=audio 23018 RTP/AVP 106 9 0 8 3 98 101
a=rtpmap:106 opus/48000/2
a=fmtp:106 sprop-maxcapturerate=16000; minptime=20; useinbandfec=1
a=rtpmap:9 G722/8000
a=rtpmap:0 PCMU/8000
a=rtpmap:8 PCMA/8000
a=rtpmap:3 GSM/8000
a=rtpmap:98 telephone-event/48000
a=fmtp:98 0-16
a=rtpmap:101 telephone-event/8000
a=fmtp:101 0-16
a=sendrecv
a=rtcp:23018
a=rtcp-mux
*/
pub fn generate_sdp() {}
