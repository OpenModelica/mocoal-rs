package Parts "Signal blocks, electrical and mechanical parts for the layout tests; no MSL"
  connector RealInput = input Real
    annotation(Icon(graphics = {Polygon(points = {{-100, 100}, {100, 0}, {-100, -100}, {-100, 100}},
      lineColor = {0, 0, 127}, fillColor = {0, 0, 127}, fillPattern = FillPattern.Solid)}));

  connector RealOutput = output Real
    annotation(Icon(graphics = {Polygon(points = {{-100, 100}, {100, 0}, {-100, -100}, {-100, 100}},
      lineColor = {0, 0, 127}, fillColor = {255, 255, 255}, fillPattern = FillPattern.Solid)}));

  connector Pin "Electrical pin"
    Real v;
    flow Real i;
    annotation(Icon(graphics = {Rectangle(extent = {{-100, 100}, {100, -100}},
      lineColor = {0, 0, 255}, fillColor = {0, 0, 255}, fillPattern = FillPattern.Solid)}));
  end Pin;

  connector Flange "Rotational flange"
    Real phi;
    flow Real tau;
    annotation(Icon(graphics = {Ellipse(extent = {{-100, 100}, {100, -100}},
      lineColor = {64, 64, 64}, fillColor = {192, 192, 192}, fillPattern = FillPattern.Solid)}));
  end Flange;

  partial block SISO "One input, one output"
    RealInput u annotation(Placement(transformation(extent = {{-140, -20}, {-100, 20}})));
    RealOutput y annotation(Placement(transformation(extent = {{100, -10}, {120, 10}})));
    annotation(Icon(graphics = {Rectangle(extent = {{-100, 100}, {100, -100}}, lineColor = {0, 0, 127})}));
  end SISO;

  block Gain
    extends SISO;
    parameter Real k = 1;
  equation
    y = k*u;
  end Gain;

  block Integrator
    extends SISO;
    parameter Real k = 1;
  equation
    der(y) = k*u;
  end Integrator;

  block FirstOrder
    extends SISO;
    parameter Real T = 1;
  equation
    T*der(y) + y = u;
  end FirstOrder;

  block Constant
    parameter Real k = 1;
    RealOutput y annotation(Placement(transformation(extent = {{100, -10}, {120, 10}})));
  equation
    y = k;
    annotation(Icon(graphics = {Rectangle(extent = {{-100, 100}, {100, -100}}, lineColor = {0, 0, 127})}));
  end Constant;

  block Sine
    parameter Real f = 1;
    RealOutput y annotation(Placement(transformation(extent = {{100, -10}, {120, 10}})));
  equation
    y = sin(2*3.14159*f*time);
    annotation(Icon(graphics = {Rectangle(extent = {{-100, 100}, {100, -100}}, lineColor = {0, 0, 127})}));
  end Sine;

  block Add
    RealInput u1 annotation(Placement(transformation(extent = {{-140, 40}, {-100, 80}})));
    RealInput u2 annotation(Placement(transformation(extent = {{-140, -80}, {-100, -40}})));
    RealOutput y annotation(Placement(transformation(extent = {{100, -10}, {120, 10}})));
  equation
    y = u1 + u2;
    annotation(Icon(graphics = {Rectangle(extent = {{-100, 100}, {100, -100}}, lineColor = {0, 0, 127})}));
  end Add;

  block Feedback "y = u1 - u2; u2 enters from below"
    RealInput u1 annotation(Placement(transformation(extent = {{-100, -20}, {-60, 20}})));
    RealInput u2 annotation(Placement(transformation(origin = {0, -80}, extent = {{-20, -20}, {20, 20}}, rotation = 90)));
    RealOutput y annotation(Placement(transformation(extent = {{80, -10}, {100, 10}})));
  equation
    y = u1 - u2;
    annotation(Icon(graphics = {Ellipse(extent = {{-20, 20}, {20, -20}}, lineColor = {0, 0, 127})}));
  end Feedback;

  partial model TwoPin
    Pin p annotation(Placement(transformation(extent = {{-110, -10}, {-90, 10}})));
    Pin n annotation(Placement(transformation(extent = {{90, -10}, {110, 10}})));
    Real v = p.v - n.v;
    Real i = p.i;
  equation
    p.i + n.i = 0;
  end TwoPin;

  model Resistor
    extends TwoPin;
    parameter Real R = 1;
  equation
    v = R*i;
    annotation(Icon(graphics = {Rectangle(extent = {{-70, 30}, {70, -30}}, lineColor = {0, 0, 255})}));
  end Resistor;

  model Capacitor
    extends TwoPin;
    parameter Real C = 1;
  equation
    i = C*der(v);
    annotation(Icon(graphics = {Line(points = {{-6, 28}, {-6, -28}}, color = {0, 0, 255}),
      Line(points = {{6, 28}, {6, -28}}, color = {0, 0, 255})}));
  end Capacitor;

  model Inductor
    extends TwoPin;
    parameter Real L = 1;
  equation
    L*der(i) = v;
    annotation(Icon(graphics = {Rectangle(extent = {{-60, 15}, {60, -15}}, lineColor = {0, 0, 255})}));
  end Inductor;

  model Ground
    Pin p annotation(Placement(transformation(origin = {0, 100}, extent = {{-10, -10}, {10, 10}}, rotation = 270)));
  equation
    p.v = 0;
    annotation(Icon(graphics = {Line(points = {{-60, 50}, {60, 50}}, color = {0, 0, 255})}));
  end Ground;

  model ConstantVoltage
    extends TwoPin;
    parameter Real V = 1;
  equation
    v = V;
    annotation(Icon(graphics = {Ellipse(extent = {{-50, 50}, {50, -50}}, lineColor = {0, 0, 255})}));
  end ConstantVoltage;

  model SignalVoltage "Voltage given by the input v_in, which enters from above"
    extends TwoPin;
    RealInput v_in annotation(Placement(transformation(origin = {0, 120}, extent = {{-20, -20}, {20, 20}}, rotation = 270)));
  equation
    v = v_in;
    annotation(Icon(graphics = {Ellipse(extent = {{-50, 50}, {50, -50}}, lineColor = {0, 0, 255})}));
  end SignalVoltage;

  model VoltageSensor
    extends TwoPin;
    RealOutput v_out annotation(Placement(transformation(origin = {0, -110}, extent = {{-10, -10}, {10, 10}}, rotation = 270)));
  equation
    i = 0;
    v_out = v;
    annotation(Icon(graphics = {Ellipse(extent = {{-50, 50}, {50, -50}}, lineColor = {0, 0, 255})}));
  end VoltageSensor;

  partial model TwoFlanges
    Flange flange_a annotation(Placement(transformation(extent = {{-110, -10}, {-90, 10}})));
    Flange flange_b annotation(Placement(transformation(extent = {{90, -10}, {110, 10}})));
  end TwoFlanges;

  model Inertia
    extends TwoFlanges;
    parameter Real J = 1;
    Real w;
  equation
    flange_a.phi = flange_b.phi;
    w = der(flange_a.phi);
    J*der(w) = flange_a.tau + flange_b.tau;
    annotation(Icon(graphics = {Rectangle(extent = {{-50, 60}, {50, -60}}, lineColor = {64, 64, 64})}));
  end Inertia;

  model Spring
    extends TwoFlanges;
    parameter Real c = 1;
  equation
    flange_b.tau = c*(flange_b.phi - flange_a.phi);
    flange_a.tau + flange_b.tau = 0;
    annotation(Icon(graphics = {Line(points = {{-60, 0}, {60, 0}}, color = {64, 64, 64})}));
  end Spring;

  model Damper
    extends TwoFlanges;
    parameter Real d = 1;
  equation
    flange_b.tau = d*der(flange_b.phi - flange_a.phi);
    flange_a.tau + flange_b.tau = 0;
    annotation(Icon(graphics = {Rectangle(extent = {{-40, 30}, {40, -30}}, lineColor = {64, 64, 64})}));
  end Damper;

  model Fixed
    Flange flange annotation(Placement(transformation(origin = {0, 100}, extent = {{-10, -10}, {10, 10}}, rotation = 270)));
  equation
    flange.phi = 0;
    annotation(Icon(graphics = {Line(points = {{-60, 50}, {60, 50}}, color = {64, 64, 64})}));
  end Fixed;

  model Torque "Torque on the flange; the input tau enters from the left"
    RealInput tau annotation(Placement(transformation(extent = {{-140, -20}, {-100, 20}})));
    Flange flange annotation(Placement(transformation(extent = {{90, -10}, {110, 10}})));
  equation
    flange.tau = -tau;
    annotation(Icon(graphics = {Rectangle(extent = {{-100, 40}, {100, -40}}, lineColor = {64, 64, 64})}));
  end Torque;

  model SpeedSensor
    Flange flange annotation(Placement(transformation(extent = {{-110, -10}, {-90, 10}})));
    RealOutput w annotation(Placement(transformation(extent = {{100, -10}, {120, 10}})));
  equation
    flange.tau = 0;
    w = der(flange.phi);
    annotation(Icon(graphics = {Ellipse(extent = {{-70, 70}, {70, -70}}, lineColor = {64, 64, 64})}));
  end SpeedSensor;
end Parts;
