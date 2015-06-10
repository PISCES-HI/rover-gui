"""
A demo python code that ..

1) Connects to an IP cam with RTSP
2) Draws RTP/NAL/H264 packets from the camera
3) Writes them to a file that can be read with any stock video player (say, mplayer, vlc & other ffmpeg based video-players)

Done for educative/demonstrative purposes, not for efficiency..!

written 2015 by Sampsa Riikonen.
"""

import base64
import socket
import re
import bitstring # if you don't have this from your linux distro, install with "pip install bitstring"

# ************************ FOR QUICK-TESTING EDIT THIS AREA *********************************************************
ip          = "10.10.153.26" # IP address of your cam
adr         = "rtsp://10.10.153.26/axis-media/media.amp" # username, passwd, etc.
target      = "rtsp://10.10.153.26/axis-media/media.amp"
clientports = [60784, 60785] # the client ports we are going to use for receiving video
fname       = "data/stream.h264" # filename for dumping the stream
rn          = 500 # receive this many packets
# After running this program, you can try your file defined in fname with "vlc fname" or "mplayer fname" from the command line
# you might also want to install h264bitstream to analyze your h264 file
# *******************************************************************************************************************

describe = "DESCRIBE "+target+" RTSP/1.0\r\nCSeq: 2\r\nUser-Agent: python\r\nAccept: application/sdp\r\n\r\n"
setup    = "SETUP "+target+"/trackID=1 RTSP/1.0\r\nCSeq: 3\r\nUser-Agent: python\r\nTransport: rtp/avp;unicast;client_port="+str(clientports[0])+"-"+str(clientports[1])+"\r\n\r\n"
play     = "PLAY "+target+" RTSP/1.0\r\nCSeq: 5\r\nUser-Agent: python\r\nSession: SESID\r\nRange: npt=0-\r\n\r\n"

last_sequence_number = 0

# File organized as follows:
# 1) Strings manipulation routines
# 2) RTP stream handling routine
# 3) Main program



# *** (1) First, some string searching/manipulation for handling the rtsp strings ***

def getPorts(searchst, st):
  """ Searching port numbers from rtsp strings using regular expressions
  """
  pat = re.compile(searchst+"=\d*-\d*")
  pat2 = re.compile('\d+')
  mstring = pat.findall(st)[0] # matched string .. "client_port=1000-1001"
  nums = pat2.findall(mstring)
  numas = []
  for num in nums:
    numas.append(int(num))
  return numas


def getLength(st):
  """ Searching "content-length" from rtsp strings using regular expressions
  """
  pat = re.compile("Content-Length: \d*")
  pat2 = re.compile('\d+')
  mstring = pat.findall(st)[0] # matched string.. "Content-Length: 614"
  num = int(pat2.findall(mstring)[0])
  return num


def printrec(recst):
  """ Pretty-printing rtsp strings
  """
  recs = recst.split('\r\n')
  for rec in recs:
    print(rec)


def sessionid(recst):
  """ Search session id from rtsp strings
  """
  recs = recst.split('\r\n')
  for rec in recs:
    ss = rec.split()
    # print(">",ss
    if (ss[0].strip()=="Session:"):
      return ss[1].split(";")[0].strip()


def setsesid(recst,idn):
  """ Sets session id in an rtsp string
  """
  return recst.replace("SESID",idn)



# ********* (2) The routine for handling the RTP stream ***********

def digestpacket(packet):
  """ This routine takes a UDP packet, i.e. a string of bytes and ..
  (a) strips off the RTP header
  (b) adds NAL "stamps" to the packets, so that they are recognized as NAL's
  (c) Concantenates frames
  (d) Returns a packet that can be written to disk as such and that is recognized by stock media players as h264 stream
  """
  global last_sequence_number
  
  startbytes = b"\x00\x00\x01" # this is the sequence of four bytes that identifies a NAL packet.. must be in front of every NAL packet.

  bt = bitstring.BitArray(bytes=packet) # turn the whole string-of-bytes packet into a string of bits.  Very unefficient, but hey, this is only for demoing.
  lc = 12 # bytecounter
  bc = 12*8 # bitcounter

  version = bt[0:2].uint # version
  p = bt[3] # P
  x = bt[4] # X
  cc = bt[4:8].uint # CC
  m = bt[9] # M
  pt = bt[9:16].uint # PT
  sn = bt[16:32].uint # sequence number
  timestamp = bt[32:64].uint # timestamp
  ssrc = bt[64:96].uint # ssrc identifier
  # The header format can be found from:
  # https://en.wikipedia.org/wiki/Real-time_Transport_Protocol

  lc = 12 # so, we have red twelve bytes
  bc = 12*8 # .. and that many bits

  print("version, p, x, cc, m, pt", version, p, x, cc, m, pt)
  print("sequence number, timestamp", sn, timestamp)
  print("sync. source identifier", ssrc)

  # packet=f.read(4*cc) # csrc identifiers, 32 bits (4 bytes) each
  cids = []
  for i in range(cc):
    cids.append(bt[bc:bc+32].uint)
    bc += 32; lc += 4;
  print("csrc identifiers:",cids)

  if (x):
    # this section haven't been tested.. might fail
    hid = bt[bc:bc+16].uint
    bc += 16; lc += 2;

    hlen = bt[bc:bc+16].uint
    bc += 16; lc += 2;

    print("ext. header id, header len",hid,hlen)

    hst = bt[bc:bc+32*hlen]
    bc += 32*hlen; lc += 4*hlen;


  # OK, now we enter the NAL packet, as described here:
  # 
  # https://tools.ietf.org/html/rfc6184#section-1.3
  #
  # Some quotes from that document:
  #
  """
  5.3. NAL Unit Header Usage


  The structure and semantics of the NAL unit header were introduced in
  Section 1.3.  For convenience, the format of the NAL unit header is
  reprinted below:

      +---------------+
      |0|1|2|3|4|5|6|7|
      +-+-+-+-+-+-+-+-+
      |F|NRI|  Type   |
      +---------------+

  This section specifies the semantics of F and NRI according to this
  specification.

  """
  """
  Table 3.  Summary of allowed NAL unit types for each packetization
                mode (yes = allowed, no = disallowed, ig = ignore)

      Payload Packet    Single NAL    Non-Interleaved    Interleaved
      Type    Type      Unit Mode           Mode             Mode
      -------------------------------------------------------------
      0      reserved      ig               ig               ig
      1-23   NAL unit     yes              yes               no
      24     STAP-A        no              yes               no
      25     STAP-B        no               no              yes
      26     MTAP16        no               no              yes
      27     MTAP24        no               no              yes
      28     FU-A          no              yes              yes
      29     FU-B          no               no              yes
      30-31  reserved      ig               ig               ig
  """
  # This was also very usefull:
  # http://stackoverflow.com/questions/7665217/how-to-process-raw-udp-packets-so-that-they-can-be-decoded-by-a-decoder-filter-i
  # A quote from that:
  """
  First byte:  [ 3 NAL UNIT BITS | 5 FRAGMENT TYPE BITS] 
  Second byte: [ START BIT | RESERVED BIT | END BIT | 5 NAL UNIT BITS] 
  Other bytes: [... VIDEO FRAGMENT DATA...]
  """

  fb = bt[bc] # i.e. "F"
  nri = bt[bc+1:bc+3].uint # "NRI"
  nlu0 = bt[bc:bc+3] # "3 NAL UNIT BITS" (i.e. [F | NRI])
  typ = bt[bc+3:bc+8].uint # "Type"
  print("F, NRI, Type :", fb, nri, typ)
  print("first three bits together :", bt[bc:bc+3])
  
  # header for output packet
  head = b""

  if typ == 1 or typ == 7 or type == 8:
    print("single NAL unit packet")
    
    if typ > 5:
      if typ == 6:
        head = b"\x00"
        print("single unit SEI")
      elif typ == 7:
        head = b"\x00"
        print("single unit PPS")
      elif typ == 8:
        head = b"\x00"
        print("single unit SPS")

    if typ == 1:
      print("single unit slice")
    if typ == 5:
      print("single unit IDR")

    head += startbytes
  elif typ == 28: # This code only handles "Type" = 28, i.e. "FU-A"
    bc += 8; # Bring bit counter to second byte
    lc += 2 # Skip first 2 bytes in packet (FU indicator (NAL unit header) and FU header)
    
    # ********* WE ARE AT THE "Second byte" ************
    # The "Type" here is most likely 28, i.e. "FU-A"
    start = bt[bc] # start bit
    end = bt[bc+1] # end bit
    # bit 2 is reserved
    nlu1 = bt[bc+3:bc+8] # 5 nal unit bits

    if start: # OK, this is a first fragment in a movie frame
      print(">>> first fragment found")
      nlu = nlu0 + nlu1 # Create "[3 NAL UNIT BITS | 5 NAL UNIT BITS]"

      head = b"\x00"
      if nlu.uint > 5:
        if nlu.uint == 6:
          head = b"\x00"
          print("fragment SEI")
        elif nlu.uint == 7:
          head = b"\x00"
          print("fragment PPS")
        elif nlu.uint == 8:
          head = b"\x00"
          print("fragment SPS")
      
      if nlu.uint == 1:
        print("fragment slice")
      if nlu.uint == 5:
        print("fragment IDR")
      
      print("fragment thing", nlu.uint)
      
      head += startbytes + nlu.bytes # .. add the NAL starting sequence
    elif not end: # intermediate fragment in a sequence, just dump "VIDEO FRAGMENT DATA"
      print("intermediate fragment found")
    elif end: # last fragment in a sequence, just dump "VIDEO FRAGMENT DATA"
      print("<<<< last fragment found")
    else:
      raise Exception("invalid FU-A start/end state")
  else:
    raise Exception("unknown frame type")
  
  if last_sequence_number == 0 or last_sequence_number < sn:
    last_sequence_number = sn
  else:
    print("skipping out of order packet")
    return b""
    raise Exception("packets out of order")
  
  return head + packet[lc:]



# *********** (3) THE MAIN PROGRAM STARTS HERE ****************

# Create an TCP socket for RTSP communication
# further reading: 
# https://docs.python.org/2.7/howto/sockets.html
s=socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.connect((ip,554)) # RTSP should peek out from port 554

print()
print("*** SENDING DESCRIBE ***")
print()
s.send(describe.encode())
recst=s.recv(4096).decode()
print()
print("*** GOT ****")
print()
printrec(recst)

print()
print("*** SENDING SETUP ***")
print()
s.send(setup.encode())
recst=s.recv(4096).decode()
print()
print("*** GOT ****")
print()
printrec(recst)
idn=sessionid(recst)

serverports=getPorts("server_port",recst)
clientports=getPorts("client_port",recst)
print("****")
print("ip,serverports",ip,serverports)
print("****")

rtp_socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
rtp_socket.bind(("", clientports[0])) # we open a port that is visible to the whole internet (the empty string "" takes care of that)
rtp_socket.settimeout(15) # if the socket is dead for 15 s., its thrown into trash
# further reading:
# https://wiki.python.org/moin/UdpCommunication

# Now our port is open for receiving shitloads of videodata.  Give the camera the PLAY command..
print()
print("*** SENDING PLAY ***")
print()
play = setsesid(play, idn)
s.send(play.encode())
recst = s.recv(4096).decode()
print()
print("*** GOT ****")
print()
printrec(recst)
print()
print()
print("** STRIPPING RTP INFO AND DUMPING INTO FILE **")
f = open(fname,'wb')
# Write SPS
f.write(b'\x00\x00\x00\x01' + base64.b64decode('Z0IAKeKQGQd/EYC3AQEBpB4kRUA='))
# Write PPS
f.write(b'\x00\x00\x00\x01' + base64.b64decode('aM48gA=='))
for i in range(rn):
  print(i)
  print()
  print()
  recst = rtp_socket.recv(4096)
  print("read", len(recst), "bytes")
  st = digestpacket(recst)
  print("dumping", len(st), "bytes")
  f.write(st)
f.close()

# Before closing the sockets, we should give the "TEARDOWN" command via RTSP, but I am feeling lazy today (after googling, wireshark-analyzing, among other-things).
s.close()
rtp_socket.close()