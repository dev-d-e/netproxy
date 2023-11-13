#[macro_export]
macro_rules! tcp_stream_start {
    ($reader:ident, $buf:ident, $n:ident , $state:ident) => {
        let _ = $reader.readable().await;
        loop {
            match $reader.try_read_buf(&mut $buf) {
                Ok(n) => {
                    trace!("tcp_stream_start {}", n);
                    if n > 0 {
                        if $buf.len() > $n {
                            break;
                        }
                        continue;
                    } else {
                        $state.0 = true;
                        break;
                    }
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        debug!("tcp_stream_start WouldBlock");
                        $state.1 = true;
                        break;
                    } else {
                        error!("tcp_stream_start error:{:?}", e);
                        $state.0 = true;
                        $state.2 = Some(e);
                        break;
                    }
                }
            }
        }
    };
}

#[macro_export]
macro_rules! tcp_stream_read {
    ($reader:ident, $buf:ident, $func:ident, $wr:ident, $state:ident) => {
        loop {
            let _ = $reader.readable().await;
            match $reader.try_read_buf(&mut $buf) {
                Ok(n) => {
                    trace!("tcp_stream_read {}", n);
                    if n > 0 {
                        if n > 1024 {
                            $func.data(&mut $buf).await;
                            if let Err(e) = $wr.write_all(&$buf).await {
                                $state.2 = true;
                                $state.3 = Some(e)
                            }
                            $buf.clear();
                        }
                        continue;
                    } else {
                        $state.0 = true;
                        break;
                    }
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        debug!("tcp_stream_read WouldBlock");
                        $state.1 = true;
                        break;
                    } else {
                        error!("tcp_stream_read error:{:?}", e);
                        $state.0 = true;
                        $state.3 = Some(e);
                        break;
                    }
                }
            }
        }
        if $buf.len() > 0 || $state.1 {
            $func.enddata(&mut $buf).await;
        }
        if $buf.len() > 0 {
            if let Err(e) = $wr.write_all(&$buf).await {
                $state.2 = true;
                $state.3 = Some(e)
            }
            $buf.clear();
        }
    };
}

#[macro_export]
macro_rules! async_ext_read {
    ($reader:ident, $buf:ident, $func:ident, $wr:ident, $state:ident) => {
        loop {
            match $reader.read_buf(&mut $buf).await {
                Ok(n) => {
                    trace!("async_ext_read {}", n);
                    if n > 0 {
                        if n > 1024 {
                            $func.data(&mut $buf).await;
                            if let Err(e) = $wr.write_all(&$buf).await {
                                $state.2 = true;
                                $state.3 = Some(e)
                            }
                            $buf.clear();
                        }
                        continue;
                    } else {
                        $state.0 = true;
                        break;
                    }
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        debug!("async_ext_read WouldBlock");
                        $state.1 = true;
                        break;
                    } else {
                        error!("async_ext_read error:{:?}", e);
                        $state.0 = true;
                        $state.3 = Some(e);
                        break;
                    }
                }
            }
        }
        if $buf.len() > 0 || $state.1 {
            $func.enddata(&mut $buf).await;
        }
        if $buf.len() > 0 {
            if let Err(e) = $wr.write_all(&$buf).await {
                $state.2 = true;
                $state.3 = Some(e)
            }
            $buf.clear();
        }
    };
}

#[macro_export]
macro_rules! async_ext_rw {
    ($server:ident, $r_buf:ident, $w_buf:ident, $func:ident, $state:ident) => {
        loop {
            match $server.read_buf(&mut $r_buf).await {
                Ok(n) => {
                    trace!("async_ext_rw {}", n);
                    if n > 0 {
                        let func = $func.clone();
                        if $r_buf.len() > 0 {
                            func.service(&mut $r_buf, &mut $w_buf).await;
                            $r_buf.clear();
                        }
                        if $w_buf.len() > 0 {
                            if let Err(e) = $server.write_all(&$w_buf).await {
                                $state.2 = true;
                                $state.3 = Some(e)
                            }
                            $w_buf.clear();
                        }
                        continue;
                    } else {
                        $state.0 = true;
                        break;
                    }
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        debug!("async_ext_rw WouldBlock");
                        $state.1 = true;
                        break;
                    } else {
                        error!("async_ext_rw error:{:?}", e);
                        $state.0 = true;
                        $state.3 = Some(e);
                        break;
                    }
                }
            }
        }
        if $r_buf.len() > 0 {
            $func.service(&mut $r_buf, &mut $w_buf).await;
            $r_buf.clear();
        }
        if $w_buf.len() > 0 {
            if let Err(e) = $server.write_all(&$w_buf).await {
                $state.2 = true;
                $state.3 = Some(e)
            }
            $w_buf.clear();
        }
    };
}
