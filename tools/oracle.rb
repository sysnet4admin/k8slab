# Vagrantfile을 실제 Ruby로 실행해 k8slab이 읽어야 할 실행 계획(JSON)을 만든다. k8slab 해석기의 정답이다.
#
#   ruby tools/oracle.rb <Vagrantfile> <arm64|x86_64>
#
# 베이그런트 대신 설정 호출만 기록하는 가짜 Vagrant.configure를 둔다. `uname -m`은 두 번째 인자로 바꾼다.
require "json"

ARCH_OVERRIDE = ARGV[1] || abort("사용법: ruby tools/oracle.rb <Vagrantfile> <arm64|x86_64>")

module Kernel
  def `(cmd)
    return "#{ARCH_OVERRIDE}\n" if cmd.strip == "uname -m"
    raise "Vagrantfile이 지원하지 않는 명령을 실행한다: #{cmd}"
  end
end

def plain(v)
  case v
  when Symbol then ":#{v}"
  when Array then v.map { |x| plain(x) }
  when Hash then v.map { |k, x| [plain(k).to_s, plain(x)] }.to_h
  else v
  end
end

class ProviderCfg
  attr_reader :data
  def initialize; @data = { "customize" => [] }; end
  def name=(v); @data["name"] = v.to_s; end
  def cpus=(v); @data["cpus"] = Integer(v); end
  def memory=(v); @data["memory"] = Integer(v); end
  def gui=(v); @data["gui"] = v ? true : false; end
  def customize(args); @data["customize"] << plain(args).map(&:to_s); end
  def method_missing(m, *a); (@data["unsupported"] ||= []) << m.to_s; end
end

class VmCfg
  attr_reader :m
  def initialize(m); @m = m; end
  def box=(v); @m["box"] = v; end
  def box_url=(v); @m["box_url"] = v; end
  def box_version=(v); @m["box_version"] = v.to_s; end
  def host_name=(v); @m["hostname"] = v; end
  alias hostname= host_name=
  def network(type, **o)
    n = { "type" => type.to_s }
    o.each do |k, v|
      n[k.to_s] = %i[guest host].include?(k) ? Integer(v) : plain(v)
    end
    @m["networks"] << n
  end
  def synced_folder(src, dst, **o)
    @m["synced_folders"] << { "src" => src, "dst" => dst, "disabled" => o[:disabled] ? true : false }
  end
  def provider(name)
    p = ProviderCfg.new
    yield p if block_given?
    @m["providers"][name.to_s] = p.data
  end
  def provision(type, **o)
    s = { "type" => type.to_s }
    o.each do |k, v|
      s[k.to_s] = case k
                  when :args then Array(v).map(&:to_s)
                  when :env then v.map { |ek, ev| [ek.to_s, ev.to_s] }.to_h
                  else plain(v)
                  end
    end
    @m["provisions"] << s
  end
  def define(name, **_o); raise "define는 config.vm에서만 부른다"; end
  def method_missing(m, *a); (@m["unsupported"] ||= []) << "vm.#{m}"; end
end

class MachineCfg
  attr_reader :vm
  def initialize(m); @vm = VmCfg.new(m); end
  def method_missing(m, *a); Ignored.new; end
end

class Ignored
  def method_missing(*_a); self; end
end

def new_machine(name)
  { "name" => name, "networks" => [], "synced_folders" => [], "providers" => {}, "provisions" => [] }
end

# define 없이 config.vm에 바로 설정하면 베이그런트처럼 "default" VM 하나로 본다.
# define과 섞어 쓰는 경우(최상위 설정이 모든 VM의 기본값)는 강의와 책에 없어 지원하지 않는다.
class RootVm
  attr_reader :default, :used
  def initialize(root); @root = root; @default = new_machine("default"); @used = false; end
  def define(name, **_o)
    m = new_machine(name.to_s)
    yield MachineCfg.new(m)
    @root.machines << m
  end
  def method_missing(m, *a, **o, &b)
    @used = true
    vm = VmCfg.new(@default)
    o.empty? ? vm.public_send(m, *a, &b) : vm.public_send(m, *a, **o, &b)
  end
end

class RootCfg
  attr_reader :machines, :vm
  def initialize; @machines = []; @vm = RootVm.new(self); end
  def method_missing(m, *a); Ignored.new; end
end

module Vagrant
  def self.configure(_version)
    root = RootCfg.new
    yield root
    machines = root.machines
    if root.vm.used
      raise "define과 최상위 config.vm 설정을 함께 쓰는 Vagrantfile은 지원하지 않는다" unless machines.empty?
      machines = [root.vm.default]
    end
    $plan = { "machines" => machines }
  end
end

path = File.expand_path(ARGV[0])
Dir.chdir(File.dirname(path)) { load path }
puts JSON.pretty_generate($plan)
