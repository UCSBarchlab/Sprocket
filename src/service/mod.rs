use rtl8139;
use timer;
use x86::shared::irq;
use fs;
use ide;
use alloc::borrow::ToOwned;

pub trait Service {
    fn name() -> &'static str; // service name
    fn start(); // called after OS has set up paging, allocator, etc.
}

pub struct UserService;

impl Service for UserService {
    fn name() -> &'static str {
        "COFFLOS HTTP Demo"
    }

    fn start() {
        println!("Reading root fs");

        let mut fs = fs::FileSystem { disk: ide::Ide::init() };

        let inum = fs.namex(b"/", b"README").unwrap();
        let inode = fs.read_inode(fs::ROOT_DEV, inum);
        match inode {
            Ok(i) => {
                println!("OK! Found 'README' at {}", inum);
                println!("Size: {}", i.size);
                println!("======================================================================");

                let mut buf = [0; fs::BLOCKSIZE];
                let mut off = 0;
                while let Ok(n) = fs.read(&i, &mut buf, off) {
                    let s = ::core::str::from_utf8(&buf[..n]);
                    match s {
                        Ok(s) => print!("{}", s),
                        Err(e) => {
                            println!("error, up to {}", e.valid_up_to());
                            println!("at offset{}. Char is '{:x}'", off, buf[e.valid_up_to()]);
                        }
                    }
                    off += fs::BLOCKSIZE as u32;
                }
                println!("======================================================================");
            }
            Err(_) => println!("Something broke :("),
        }

        use alloc::string::String;

        let inum = fs.namex(b"/", b"small.html").unwrap();
        let inode = fs.read_inode(fs::ROOT_DEV, inum);
        let html = match inode {
            Ok(i) => {
                let mut buf = vec![0; i.size as usize];
                fs.read(&i, &mut buf, 0).unwrap();
                String::from_utf8(buf).unwrap().replace("${{VERSION}}", env!("CARGO_PKG_VERSION"))
            }
            Err(_) => panic!("Couldn't load HTML file"),
        };

        let header: String = "HTTP/1.1 200 OK\r\n\r\n".to_owned();
        let http = header + html.as_str();

        unsafe { irq::enable() };
        loop {
            use smoltcp::iface::{EthernetInterface, SliceArpCache, ArpCache};
            use smoltcp::wire::{EthernetAddress, IpAddress};
            use smoltcp::socket::{AsSocket, SocketSet};
            use smoltcp::socket::{TcpSocket, TcpSocketBuffer};
            use smoltcp::Error;
            use alloc::boxed::Box;
            use core::str;

            let arp_cache = SliceArpCache::new(vec![Default::default(); 8]);
            let hw_addr = unsafe { EthernetAddress(rtl8139::NIC.as_mut().unwrap().mac_address()) };

            let protocol_addr = IpAddress::v4(10, 0, 0, 4);
            let nic = unsafe { rtl8139::NIC.as_mut().unwrap() };
            let mut iface = EthernetInterface::new(nic,
                                                   Box::new(arp_cache) as Box<ArpCache>,
                                                   hw_addr,
                                                   [protocol_addr]);

            let tcp_rx_buffer = TcpSocketBuffer::new(vec![0; 4096]);
            let tcp_tx_buffer = TcpSocketBuffer::new(vec![0; 4096]);
            let tcp_socket = TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer);

            let mut sockets = SocketSet::new(vec![]);
            let tcp_handle = sockets.add(tcp_socket);

            loop {
                {
                    let socket: &mut TcpSocket = sockets.get_mut(tcp_handle).as_socket();
                    if !socket.is_open() {
                        socket.listen(80).unwrap();
                    }

                    if socket.can_recv() {
                        let _ = socket.recv(200);
                        if socket.can_send() {
                            let seconds = unsafe { timer::TICKS } / 100;
                            const SECONDS_PER_MINUTE: u32 = 60;
                            const MINUTES_PER_HOUR: u32 = 60;
                            const HOURS_PER_DAY: u32 = 24;
                            const SECONDS_PER_DAY: u32 = (SECONDS_PER_MINUTE * MINUTES_PER_HOUR *
                                                          HOURS_PER_DAY);
                            const SECONDS_PER_HOUR: u32 = (SECONDS_PER_MINUTE * MINUTES_PER_HOUR);

                            let days = seconds / SECONDS_PER_DAY;
                            let hours = (seconds % SECONDS_PER_DAY) / SECONDS_PER_HOUR;
                            let minutes = (seconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE;
                            let seconds = seconds % SECONDS_PER_MINUTE;

                            let time =
                                format!("{} days, {}:{:02}:{:02}", days, hours, minutes, seconds);

                            socket.send_slice(http.replace("${{TIME}}", &time).as_str().as_bytes())
                                .unwrap();
                            println!("socket closing");
                            socket.close();
                        }
                    }
                }

                match iface.poll(&mut sockets, 10) {
                    Ok(()) | Err(Error::Exhausted) => (),
                    Err(e) => println!("poll error: {}", e),
                }
            }

        }

    }
}
